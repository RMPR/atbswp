//! Turns raw touchpad frames into the clicks libinput would synthesise.
//!
//! evdev never reports a tap-to-click as a button: the pad only says that a
//! finger touched, how many fingers rest on it and where they are, and
//! libinput (inside the compositor) decides that a short, still touch was a
//! click.  Likewise a physical press of a clickpad is always `BTN_LEFT`; a
//! two- or three-finger press is turned into a right or middle click by
//! libinput's clickfinger method, the default on Apple touchpads.
//!
//! This state machine reproduces both rules with libinput's defaults: a tap
//! is a touch that ends within 180 ms, having moved less than about 2 mm,
//! with no physical press in between; finger count selects the button.

use atbswp_macro::format::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT};

pub const BTN_TOOL_FINGER: u16 = 0x145;
pub const BTN_TOUCH: u16 = 0x14a;
pub const BTN_TOOL_DOUBLETAP: u16 = 0x14d;
pub const BTN_TOOL_TRIPLETAP: u16 = 0x14e;
pub const BTN_TOOL_QUADTAP: u16 = 0x14f;
pub const ABS_X: u16 = 0;
pub const ABS_Y: u16 = 1;

const TAP_TIMEOUT_US: u64 = 180_000;

/// A synthesised click: press and release of `code`, both at `t_us`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    Press(u16),
    Release(u16),
}

pub struct Touchpad {
    /// motion threshold in device units (about 2 mm)
    move_threshold: i32,
    fingers: u32,
    /// most fingers seen during the current touch (they lift in the same
    /// frame the touch ends, so the live count is 0 by then)
    peak_fingers: u32,
    touching: bool,
    touch_start_us: u64,
    start: Option<(i32, i32)>,
    cur: (i32, i32),
    moved: bool,
    clicked_during_touch: bool,
    /// button code we reported for a physical press, so its release matches
    physical: Option<u16>,
    /// per-frame buffer: (code, pressed) of button-ish keys
    frame_keys: Vec<(u16, bool)>,
    verbose: bool,
}

fn button_for(fingers: u32) -> u16 {
    match fingers {
        2 => BTN_RIGHT,
        3 => BTN_MIDDLE,
        _ => BTN_LEFT,
    }
}

impl Touchpad {
    /// `units_per_mm` comes from the pad's ABS resolution; 0 = unknown.
    pub fn new(units_per_mm: i32, abs_range: i32) -> Self {
        let move_threshold = if units_per_mm > 0 {
            units_per_mm * 2
        } else {
            (abs_range / 50).max(1)
        };
        Touchpad {
            move_threshold,
            fingers: 0,
            peak_fingers: 0,
            touching: false,
            touch_start_us: 0,
            start: None,
            cur: (0, 0),
            moved: false,
            clicked_during_touch: false,
            physical: None,
            frame_keys: vec![],
            verbose: std::env::var_os("ATBSWP_VERBOSE").is_some(),
        }
    }

    /// One line for `ATBSWP_VERBOSE` diagnostics.
    pub fn describe(&self) -> String {
        format!(
            "tap-to-click and clickfinger synthesis on, motion threshold {} units",
            self.move_threshold
        )
    }

    pub fn is_tool_key(code: u16) -> bool {
        matches!(
            code,
            BTN_TOUCH
                | BTN_TOOL_FINGER
                | BTN_TOOL_DOUBLETAP
                | BTN_TOOL_TRIPLETAP
                | BTN_TOOL_QUADTAP
        )
    }

    /// A touchpad-related EV_KEY (tool/touch keys or BTN_LEFT..) inside a frame.
    pub fn key(&mut self, code: u16, pressed: bool) {
        self.frame_keys.push((code, pressed));
    }

    pub fn abs(&mut self, code: u16, value: i32) {
        match code {
            ABS_X => self.cur.0 = value,
            ABS_Y => self.cur.1 = value,
            _ => return,
        }
        if let Some((sx, sy)) = self.start
            && ((self.cur.0 - sx).abs() > self.move_threshold
                || (self.cur.1 - sy).abs() > self.move_threshold)
        {
            self.moved = true;
        }
    }

    /// End of frame (EV_SYN): returns the clicks to emit, in order.
    pub fn frame(&mut self, t_us: u64) -> Vec<Out> {
        let mut out = vec![];
        let keys = std::mem::take(&mut self.frame_keys);
        // 1. finger count first, so a physical press in the same frame sees it
        for &(code, pressed) in &keys {
            let n = match code {
                BTN_TOOL_FINGER => 1,
                BTN_TOOL_DOUBLETAP => 2,
                BTN_TOOL_TRIPLETAP => 3,
                BTN_TOOL_QUADTAP => 4,
                _ => continue,
            };
            if pressed {
                self.fingers = n;
            } else if self.fingers == n {
                self.fingers = 0;
            }
        }
        if self.touching {
            self.peak_fingers = self.peak_fingers.max(self.fingers);
        }
        for &(code, pressed) in &keys {
            match code {
                BTN_TOUCH if pressed && !self.touching => {
                    self.touching = true;
                    self.touch_start_us = t_us;
                    self.peak_fingers = self.fingers.max(1);
                    self.start = Some(self.cur);
                    self.moved = false;
                    self.clicked_during_touch = false;
                }
                BTN_TOUCH if !pressed && self.touching => {
                    self.touching = false;
                    let held_us = t_us.saturating_sub(self.touch_start_us);
                    let quick = held_us <= TAP_TIMEOUT_US;
                    if quick && !self.moved && !self.clicked_during_touch {
                        let b = button_for(self.peak_fingers.max(1));
                        out.push(Out::Press(b));
                        out.push(Out::Release(b));
                    } else if self.verbose {
                        eprintln!(
                            "atbswp: touch ignored (not a tap): {}ms, moved={}, physical click={}, fingers={}",
                            held_us / 1000,
                            self.moved,
                            self.clicked_during_touch,
                            self.peak_fingers
                        );
                    }
                    self.start = None;
                }
                BTN_LEFT if pressed => {
                    // physical clickpad press: clickfinger mapping
                    self.clicked_during_touch = true;
                    let b = button_for(self.fingers.max(1));
                    self.physical = Some(b);
                    out.push(Out::Press(b));
                }
                BTN_LEFT if !pressed => {
                    if let Some(b) = self.physical.take() {
                        out.push(Out::Release(b));
                    }
                }
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad() -> Touchpad {
        Touchpad::new(10, 1000) // 10 units/mm -> 20 unit threshold
    }

    #[test]
    fn one_finger_tap_is_left_click() {
        let mut p = pad();
        p.abs(ABS_X, 500);
        p.abs(ABS_Y, 500);
        p.key(BTN_TOUCH, true);
        p.key(BTN_TOOL_FINGER, true);
        assert!(p.frame(0).is_empty());
        p.abs(ABS_X, 505); // jitter below threshold
        assert!(p.frame(50_000).is_empty());
        p.key(BTN_TOUCH, false);
        p.key(BTN_TOOL_FINGER, false);
        assert_eq!(
            p.frame(100_000),
            vec![Out::Press(BTN_LEFT), Out::Release(BTN_LEFT)]
        );
    }

    #[test]
    fn two_finger_tap_is_right_click_and_slow_or_moving_touch_is_not_a_click() {
        let mut p = pad();
        p.key(BTN_TOUCH, true);
        p.key(BTN_TOOL_DOUBLETAP, true);
        p.frame(0);
        p.key(BTN_TOUCH, false);
        p.key(BTN_TOOL_DOUBLETAP, false);
        assert_eq!(
            p.frame(120_000),
            vec![Out::Press(BTN_RIGHT), Out::Release(BTN_RIGHT)]
        );

        // too slow
        p.key(BTN_TOUCH, true);
        p.key(BTN_TOOL_FINGER, true);
        p.frame(1_000_000);
        p.key(BTN_TOUCH, false);
        p.key(BTN_TOOL_FINGER, false);
        assert!(p.frame(1_400_000).is_empty());

        // moved: a pointer motion, not a tap
        p.abs(ABS_X, 100);
        p.key(BTN_TOUCH, true);
        p.key(BTN_TOOL_FINGER, true);
        p.frame(2_000_000);
        p.abs(ABS_X, 200);
        p.frame(2_050_000);
        p.key(BTN_TOUCH, false);
        p.key(BTN_TOOL_FINGER, false);
        assert!(p.frame(2_100_000).is_empty());
    }

    #[test]
    fn physical_press_with_two_fingers_is_right_click_and_suppresses_tap() {
        let mut p = pad();
        p.key(BTN_TOUCH, true);
        p.key(BTN_TOOL_DOUBLETAP, true);
        p.key(BTN_LEFT, true); // same frame as the fingers landing
        assert_eq!(p.frame(0), vec![Out::Press(BTN_RIGHT)]);
        p.key(BTN_LEFT, false);
        assert_eq!(p.frame(80_000), vec![Out::Release(BTN_RIGHT)]);
        p.key(BTN_TOUCH, false);
        p.key(BTN_TOOL_DOUBLETAP, false);
        assert!(p.frame(100_000).is_empty()); // lift-off is not a second click
    }

    #[test]
    fn plain_physical_click_is_left() {
        let mut p = pad();
        p.key(BTN_TOUCH, true);
        p.key(BTN_TOOL_FINGER, true);
        p.frame(0);
        p.key(BTN_LEFT, true);
        assert_eq!(p.frame(300_000), vec![Out::Press(BTN_LEFT)]);
        p.key(BTN_LEFT, false);
        assert_eq!(p.frame(400_000), vec![Out::Release(BTN_LEFT)]);
    }
}
