//! X11 recorder: the XRecord extension delivers every core input event the
//! server processes, including XTest-synthesised ones, to an unprivileged
//! client.  Absolute pointer positions come for free.

use super::{Builder, Options};
use atbswp_macro::{Header, Macro, format};
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::record::{self, ConnectionExt as _};
use x11rb::protocol::xproto;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::x11_utils::TryParse;

const START_OF_DATA: u8 = 4;
const RECORD_FROM_SERVER: u8 = 0;

pub fn record(opts: &Options) -> Result<Macro, String> {
    let (ctrl, screen_num) = x11rb::connect(None).map_err(|e| format!("X11: {e}"))?;
    let (data, _) = x11rb::connect(None).map_err(|e| format!("X11: {e}"))?;
    if ctrl
        .extension_information(record::X11_EXTENSION_NAME)
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("the X server lacks the RECORD extension".into());
    }
    let screen = &ctrl.setup().roots[screen_num];
    let (screen_w, screen_h) = opts.screen.unwrap_or((
        screen.width_in_pixels as u32,
        screen.height_in_pixels as u32,
    ));

    let rc = ctrl.generate_id().map_err(|e| e.to_string())?;
    let empty = record::Range8 { first: 0, last: 0 };
    let empty_ext = record::ExtRange {
        major: empty,
        minor: record::Range16 { first: 0, last: 0 },
    };
    let range = record::Range {
        core_requests: empty,
        core_replies: empty,
        ext_requests: empty_ext,
        ext_replies: empty_ext,
        delivered_events: empty,
        device_events: record::Range8 {
            first: xproto::KEY_PRESS_EVENT,
            last: xproto::MOTION_NOTIFY_EVENT,
        },
        errors: empty,
        client_started: false,
        client_died: false,
    };
    ctrl.record_create_context(rc, 0, &[record::CS::ALL_CLIENTS.into()], &[range])
        .map_err(|e| e.to_string())?
        .check()
        .map_err(|e| format!("RecordCreateContext: {e}"))?;

    // request_stop() from another thread (or SIGINT in the CLI) is turned
    // into a RecordDisableContext, which ends the reply stream below.
    let stopper = {
        let ctrl = std::sync::Arc::new(ctrl);
        let c = ctrl.clone();
        let handle_signals = opts.handle_signals;
        std::thread::spawn(move || {
            if handle_signals {
                unsafe {
                    libc::signal(libc::SIGINT, on_sigint as *const () as usize);
                    libc::signal(libc::SIGTERM, on_sigint as *const () as usize);
                }
            }
            while !super::stop_requested() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            let _ = c.record_disable_context(rc);
            let _ = c.sync();
        });
        ctrl
    };
    let _ = &stopper;

    eprintln!(
        "atbswp: recording via XRecord; press {} to stop",
        opts.stop_key
            .and_then(atbswp_macro::keys::key_name)
            .unwrap_or("Ctrl-C")
    );
    let mut clock = ServerClock::default();
    let mut b = Builder::new(opts);
    let replies = data.record_enable_context(rc).map_err(|e| e.to_string())?;
    'outer: for reply in replies {
        let reply = reply.map_err(|e| e.to_string())?;
        if reply.category == START_OF_DATA || reply.client_swapped {
            continue;
        }
        if reply.category != RECORD_FROM_SERVER {
            continue;
        }
        let mut d = &reply.data[..];
        while !d.is_empty() {
            let (rest, stop) =
                handle(&mut b, &mut clock, d).map_err(|e| format!("XRecord parse: {e}"))?;
            if stop {
                super::request_stop();
                break 'outer;
            }
            d = rest;
        }
    }
    super::request_stop(); // let the stopper thread finish
    Ok(b.finish(Header {
        screen_w,
        screen_h,
        ..Default::default()
    }))
}

extern "C" fn on_sigint(_: libc::c_int) {
    super::request_stop();
}

fn button_event(b: &mut Builder, now_us: u64, detail: u8, pressed: bool) {
    match detail {
        1 => b.button(now_us, format::BTN_LEFT, pressed),
        2 => b.button(now_us, format::BTN_MIDDLE, pressed),
        3 => b.button(now_us, format::BTN_RIGHT, pressed),
        4 if pressed => b.scroll(now_us, 0, -format::SCROLL_NOTCH),
        5 if pressed => b.scroll(now_us, 0, format::SCROLL_NOTCH),
        6 if pressed => b.scroll(now_us, -format::SCROLL_NOTCH, 0),
        7 if pressed => b.scroll(now_us, format::SCROLL_NOTCH, 0),
        8 => b.button(now_us, format::BTN_SIDE, pressed),
        9 => b.button(now_us, format::BTN_EXTRA, pressed),
        _ => {}
    }
}

/// Converts X server timestamps (ms, wrapping at 2^32) into a monotonic µs clock.
#[derive(Default)]
struct ServerClock {
    last_ms: Option<u32>,
    us: u64,
}

impl ServerClock {
    fn at(&mut self, ms: u32) -> u64 {
        if let Some(last) = self.last_ms {
            self.us += (ms.wrapping_sub(last) as u64) * 1000;
        }
        self.last_ms = Some(ms);
        self.us
    }
}

fn handle<'a>(
    b: &mut Builder,
    clock: &mut ServerClock,
    data: &'a [u8],
) -> Result<(&'a [u8], bool), x11rb::errors::ParseError> {
    if std::env::var_os("ATBSWP_DEBUG").is_some() {
        eprintln!("xrecord: event code {} ({} bytes)", data[0], data.len());
    }
    Ok(match data[0] & 0x7f {
        xproto::KEY_PRESS_EVENT | xproto::KEY_RELEASE_EVENT => {
            let (ev, rest) = xproto::KeyPressEvent::try_parse(data)?;
            let pressed = data[0] & 0x7f == xproto::KEY_PRESS_EVENT;
            let now_us = clock.at(ev.time);
            let stop = ev.detail >= 8 && b.key(now_us, (ev.detail - 8) as u16, pressed);
            (rest, stop)
        }
        xproto::BUTTON_PRESS_EVENT | xproto::BUTTON_RELEASE_EVENT => {
            let (ev, rest) = xproto::ButtonPressEvent::try_parse(data)?;
            let now_us = clock.at(ev.time);
            button_event(
                b,
                now_us,
                ev.detail,
                data[0] & 0x7f == xproto::BUTTON_PRESS_EVENT,
            );
            (rest, false)
        }
        xproto::MOTION_NOTIFY_EVENT => {
            let (ev, rest) = xproto::MotionNotifyEvent::try_parse(data)?;
            let now_us = clock.at(ev.time);
            b.move_abs(now_us, ev.root_x as i32, ev.root_y as i32);
            (rest, false)
        }
        0 => {
            // a reply: skip by its declared length
            let (len, _) = u32::try_parse(&data[4..])?;
            let len = len as usize * 4 + 32;
            (&data[len.min(data.len())..], false)
        }
        _ => (&data[32.min(data.len())..], false), // other events are 32 bytes
    })
}
