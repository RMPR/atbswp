//! Windows recorder: low-level keyboard and mouse hooks.  Unprivileged, sees
//! injected input too, delivers absolute pointer positions.

use super::{Builder, Options, keymap};
use atbswp_macro::{Header, Macro, format};
use std::sync::Mutex;
use std::time::Instant;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

struct State {
    b: Builder,
    t0: Instant,
    stopped: bool,
    unmapped: u32,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with_state(f: impl FnOnce(&mut State, u64)) {
    if let Ok(mut g) = STATE.lock() {
        if let Some(st) = g.as_mut() {
            let now = st.t0.elapsed().as_micros() as u64;
            f(st, now);
        }
    }
}

unsafe extern "system" fn kb_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let k = unsafe { &*(l as *const KBDLLHOOKSTRUCT) };
        let ext = k.flags & LLKHF_EXTENDED != 0;
        let up = k.flags & LLKHF_UP != 0;
        with_state(
            |st, now| match keymap::evdev_from_win(k.scanCode as u8, ext) {
                Some(code) => {
                    if st.b.key(now, code, !up) {
                        st.stopped = true;
                        super::request_stop();
                    }
                }
                None => st.unmapped += 1,
            },
        );
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w, l) }
}

unsafe extern "system" fn ms_hook(code: i32, w: WPARAM, l: LPARAM) -> LRESULT {
    if code >= 0 {
        let m = unsafe { &*(l as *const MSLLHOOKSTRUCT) };
        let hi = ((m.mouseData >> 16) & 0xffff) as u16 as i16 as i32;
        with_state(|st, now| match w as u32 {
            WM_MOUSEMOVE => st.b.move_abs(now, m.pt.x, m.pt.y),
            WM_LBUTTONDOWN => st.b.button(now, format::BTN_LEFT, true),
            WM_LBUTTONUP => st.b.button(now, format::BTN_LEFT, false),
            WM_RBUTTONDOWN => st.b.button(now, format::BTN_RIGHT, true),
            WM_RBUTTONUP => st.b.button(now, format::BTN_RIGHT, false),
            WM_MBUTTONDOWN => st.b.button(now, format::BTN_MIDDLE, true),
            WM_MBUTTONUP => st.b.button(now, format::BTN_MIDDLE, false),
            WM_XBUTTONDOWN | WM_XBUTTONUP => {
                let code = if hi == 1 {
                    format::BTN_SIDE
                } else {
                    format::BTN_EXTRA
                };
                st.b.button(now, code, w as u32 == WM_XBUTTONDOWN);
            }
            WM_MOUSEWHEEL => st.b.scroll(now, 0, -hi), // WHEEL_DELTA units == ours
            WM_MOUSEHWHEEL => st.b.scroll(now, hi, 0),
            _ => {}
        });
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, w, l) }
}

pub fn record(opts: &Options) -> Result<Macro, String> {
    let (screen_w, screen_h) = opts.screen.unwrap_or_else(|| unsafe {
        (
            GetSystemMetrics(SM_CXVIRTUALSCREEN) as u32,
            GetSystemMetrics(SM_CYVIRTUALSCREEN) as u32,
        )
    });
    *STATE.lock().unwrap() = Some(State {
        b: Builder::new(opts),
        t0: Instant::now(),
        stopped: false,
        unmapped: 0,
    });

    let (kb, ms) = unsafe {
        (
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_hook), std::ptr::null_mut(), 0),
            SetWindowsHookExW(WH_MOUSE_LL, Some(ms_hook), std::ptr::null_mut(), 0),
        )
    };
    if kb.is_null() || ms.is_null() {
        *STATE.lock().unwrap() = None;
        return Err("SetWindowsHookEx failed (is there an interactive desktop?)".into());
    }
    eprintln!(
        "atbswp: recording via low-level hooks; press {} to stop",
        opts.stop_key
            .and_then(atbswp_macro::keys::key_name)
            .unwrap_or("the stop key")
    );

    // Low-level hooks are only delivered to a thread that pumps messages.
    while !super::stop_requested() {
        unsafe {
            MsgWaitForMultipleObjects(0, std::ptr::null(), 0, 100, QS_ALLINPUT);
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
    unsafe {
        UnhookWindowsHookEx(kb);
        UnhookWindowsHookEx(ms);
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
