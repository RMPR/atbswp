//! macOS recorder: a listen-only CGEventTap on the session.  Needs the
//! Input Monitoring permission (macOS prompts once, per application).

use super::{Builder, Options, keymap};
use atbswp_macro::{Header, Macro, format};
use std::ffi::c_void;
use std::sync::Mutex;
use std::time::Instant;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

type CGEventRef = *mut c_void;
type CGEventTapCallBack = extern "C" fn(*mut c_void, u32, CGEventRef, *mut c_void) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        mask: u64,
        cb: CGEventTapCallBack,
        user: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CGEventGetLocation(e: CGEventRef) -> CGPoint;
    fn CGEventGetIntegerValueField(e: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(e: CGEventRef) -> u64;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayPixelsWide(d: u32) -> usize;
    fn CGDisplayPixelsHigh(d: u32) -> usize;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: *const c_void;
    fn CFMachPortCreateRunLoopSource(
        alloc: *const c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, src: *mut c_void, mode: *const c_void);
    fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after_source: bool) -> i32;
    fn CFRelease(p: *const c_void);
}

const K_CG_SESSION_EVENT_TAP: u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;

const LEFT_DOWN: u32 = 1;
const LEFT_UP: u32 = 2;
const RIGHT_DOWN: u32 = 3;
const RIGHT_UP: u32 = 4;
const MOUSE_MOVED: u32 = 5;
const LEFT_DRAGGED: u32 = 6;
const RIGHT_DRAGGED: u32 = 7;
const KEY_DOWN: u32 = 10;
const KEY_UP: u32 = 11;
const FLAGS_CHANGED: u32 = 12;
const SCROLL_WHEEL: u32 = 22;
const OTHER_DOWN: u32 = 25;
const OTHER_UP: u32 = 26;
const OTHER_DRAGGED: u32 = 27;
const TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;
const TAP_DISABLED_BY_USER: u32 = 0xFFFF_FFFF;

const FIELD_MOUSE_BUTTON_NUMBER: u32 = 3;
const FIELD_KEYBOARD_AUTOREPEAT: u32 = 8;
const FIELD_KEYBOARD_KEYCODE: u32 = 9;
const FIELD_SCROLL_DELTA_AXIS1: u32 = 11;
const FIELD_SCROLL_DELTA_AXIS2: u32 = 12;

struct State {
    b: Builder,
    t0: Instant,
    held_modifiers: Vec<u16>,
    unmapped: u32,
    tap: *mut c_void,
}
unsafe impl Send for State {}

static STATE: Mutex<Option<State>> = Mutex::new(None);

extern "C" fn tap_callback(
    _proxy: *mut c_void,
    kind: u32,
    event: CGEventRef,
    _user: *mut c_void,
) -> CGEventRef {
    let Ok(mut g) = STATE.lock() else {
        return event;
    };
    let Some(st) = g.as_mut() else { return event };
    let now = st.t0.elapsed().as_micros() as u64;
    unsafe {
        match kind {
            TAP_DISABLED_BY_TIMEOUT | TAP_DISABLED_BY_USER => CGEventTapEnable(st.tap, true),
            MOUSE_MOVED | LEFT_DRAGGED | RIGHT_DRAGGED | OTHER_DRAGGED => {
                let p = CGEventGetLocation(event);
                st.b.move_abs(now, p.x as i32, p.y as i32);
            }
            LEFT_DOWN => st.b.button(now, format::BTN_LEFT, true),
            LEFT_UP => st.b.button(now, format::BTN_LEFT, false),
            RIGHT_DOWN => st.b.button(now, format::BTN_RIGHT, true),
            RIGHT_UP => st.b.button(now, format::BTN_RIGHT, false),
            OTHER_DOWN | OTHER_UP => {
                let code = match CGEventGetIntegerValueField(event, FIELD_MOUSE_BUTTON_NUMBER) {
                    2 => format::BTN_MIDDLE,
                    3 => format::BTN_SIDE,
                    _ => format::BTN_EXTRA,
                };
                st.b.button(now, code, kind == OTHER_DOWN);
            }
            SCROLL_WHEEL => {
                // line deltas: positive = up/left in CoreGraphics
                let dy = CGEventGetIntegerValueField(event, FIELD_SCROLL_DELTA_AXIS1) as i32;
                let dx = CGEventGetIntegerValueField(event, FIELD_SCROLL_DELTA_AXIS2) as i32;
                if dx != 0 || dy != 0 {
                    st.b.scroll(now, -dx * format::SCROLL_NOTCH, -dy * format::SCROLL_NOTCH);
                }
            }
            KEY_DOWN | KEY_UP => {
                if CGEventGetIntegerValueField(event, FIELD_KEYBOARD_AUTOREPEAT) != 0 {
                    return event;
                }
                let vk = CGEventGetIntegerValueField(event, FIELD_KEYBOARD_KEYCODE) as u8;
                match keymap::evdev_from_mac(vk) {
                    Some(code) => {
                        if st.b.key(now, code, kind == KEY_DOWN) {
                            super::request_stop();
                        }
                    }
                    None => st.unmapped += 1,
                }
            }
            FLAGS_CHANGED => {
                // Modifiers arrive as flag changes; toggle press/release per keycode.
                let _ = CGEventGetFlags(event);
                let vk = CGEventGetIntegerValueField(event, FIELD_KEYBOARD_KEYCODE) as u8;
                if let Some(code) = keymap::evdev_from_mac(vk) {
                    let pressed = !st.held_modifiers.contains(&code);
                    if pressed {
                        st.held_modifiers.push(code);
                    } else {
                        st.held_modifiers.retain(|c| *c != code);
                    }
                    st.b.key(now, code, pressed);
                }
            }
            _ => {}
        }
    }
    event
}

pub fn record(opts: &Options) -> Result<Macro, String> {
    unsafe {
        if !CGPreflightListenEventAccess() {
            eprintln!(
                "atbswp: requesting Input Monitoring permission (System Settings > Privacy & Security)"
            );
            if !CGRequestListenEventAccess() {
                return Err(
                    "Input Monitoring permission is required to record; grant it and run again"
                        .into(),
                );
            }
        }
    }
    let (screen_w, screen_h) = opts.screen.unwrap_or_else(|| unsafe {
        let d = CGMainDisplayID();
        (CGDisplayPixelsWide(d) as u32, CGDisplayPixelsHigh(d) as u32)
    });
    let mask: u64 = [
        LEFT_DOWN,
        LEFT_UP,
        RIGHT_DOWN,
        RIGHT_UP,
        MOUSE_MOVED,
        LEFT_DRAGGED,
        RIGHT_DRAGGED,
        KEY_DOWN,
        KEY_UP,
        FLAGS_CHANGED,
        SCROLL_WHEEL,
        OTHER_DOWN,
        OTHER_UP,
        OTHER_DRAGGED,
    ]
    .iter()
    .fold(0, |m, t| m | (1u64 << t));

    *STATE.lock().unwrap() = Some(State {
        b: Builder::new(opts),
        t0: Instant::now(),
        held_modifiers: vec![],
        unmapped: 0,
        tap: std::ptr::null_mut(),
    });
    let tap = unsafe {
        CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
            mask,
            tap_callback,
            std::ptr::null_mut(),
        )
    };
    if tap.is_null() {
        *STATE.lock().unwrap() = None;
        return Err("CGEventTapCreate failed: Input Monitoring permission missing?".into());
    }
    STATE.lock().unwrap().as_mut().unwrap().tap = tap;
    unsafe {
        let src = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        CFRunLoopAddSource(CFRunLoopGetCurrent(), src, kCFRunLoopDefaultMode);
        CGEventTapEnable(tap, true);
        eprintln!(
            "atbswp: recording via CGEventTap; press {} to stop",
            opts.stop_key
                .and_then(atbswp_macro::keys::key_name)
                .unwrap_or("the stop key")
        );
        while !super::stop_requested() {
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
        }
        CGEventTapEnable(tap, false);
        CFRelease(src);
        CFRelease(tap);
    }
    let st = STATE
        .lock()
        .unwrap()
        .take()
        .ok_or("recorder state vanished")?;
    if st.unmapped > 0 {
        eprintln!(
            "atbswp: {} key events had no evdev mapping and were skipped",
            st.unmapped
        );
    }
    Ok(st.b.finish(Header {
        screen_w,
        screen_h,
        ..Default::default()
    }))
}
