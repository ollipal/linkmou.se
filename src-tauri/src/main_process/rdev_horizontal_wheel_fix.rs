// PR opened: https://github.com/Narsil/rdev/pull/106

pub const TRUE: c_int = 1;
pub const FALSE: c_int = 0;

use rdev::{Key, Button, EventType, SimulateError};
use std::os::raw::c_uint;

macro_rules! decl_keycodes {
    ($($key:ident, $code:literal),*) => {
        //TODO: make const when rust lang issue #49146 is fixed
        pub fn code_from_key(key: Key) -> Option<c_uint> {
            match key {
                $(
                    Key::$key => Some($code),
                )*
                Key::Unknown(code) => Some(code),
                _ => None,
            }
        }

        //TODO: make const when rust lang issue #49146 is fixed
        pub fn key_from_code(code: c_uint) -> Key {
            match code {
                $(
                    $code => Key::$key,
                )*
                _ => Key::Unknown(code)
            }
        }
    };
}

#[rustfmt::skip]
decl_keycodes!(
    Alt, 64,
    AltGr, 108,
    Backspace, 22,
    CapsLock, 66,
    ControlLeft, 37,
    ControlRight, 105,
    Delete, 119,
    DownArrow, 116,
    End, 115,
    Escape, 9,
    F1, 67,
    F10, 76,
    F11, 95,
    F12, 96,
    F2, 68,
    F3, 69,
    F4, 70,
    F5, 71,
    F6, 72,
    F7, 73,
    F8, 74,
    F9, 75,
    Home, 110,
    LeftArrow, 113,
    MetaLeft, 133,
    PageDown, 117,
    PageUp, 112,
    Return, 36,
    RightArrow, 114,
    ShiftLeft, 50,
    ShiftRight, 62,
    Space, 65,
    Tab, 23,
    UpArrow, 111,
    PrintScreen, 107,
    ScrollLock, 78,
    Pause, 127,
    NumLock, 77,
    BackQuote, 49,
    Num1, 10,
    Num2, 11,
    Num3, 12,
    Num4, 13,
    Num5, 14,
    Num6, 15,
    Num7, 16,
    Num8, 17,
    Num9, 18,
    Num0, 19,
    Minus, 20,
    Equal, 21,
    KeyQ, 24,
    KeyW, 25,
    KeyE, 26,
    KeyR, 27,
    KeyT, 28,
    KeyY, 29,
    KeyU, 30,
    KeyI, 31,
    KeyO, 32,
    KeyP, 33,
    LeftBracket, 34,
    RightBracket, 35,
    KeyA, 38,
    KeyS, 39,
    KeyD, 40,
    KeyF, 41,
    KeyG, 42,
    KeyH, 43,
    KeyJ, 44,
    KeyK, 45,
    KeyL, 46,
    SemiColon, 47,
    Quote, 48,
    BackSlash, 51,
    IntlBackslash, 94,
    KeyZ, 52,
    KeyX, 53,
    KeyC, 54,
    KeyV, 55,
    KeyB, 56,
    KeyN, 57,
    KeyM, 58,
    Comma, 59,
    Dot, 60,
    Slash, 61,
    Insert, 118,
    KpReturn, 104,
    KpMinus, 82,
    KpPlus, 86,
    KpMultiply, 63,
    KpDivide, 106,
    Kp0, 90,
    Kp1, 87,
    Kp2, 88,
    Kp3, 89,
    Kp4, 83,
    Kp5, 84,
    Kp6, 85,
    Kp7, 79,
    Kp8, 80,
    Kp9, 81,
    KpDelete, 91
);

use std::convert::TryInto;
use std::os::raw::c_int;
use std::ptr::null;
use x11::xlib;
use x11::xtest;

unsafe fn send_native(event_type: &EventType, display: *mut xlib::Display) -> Option<()> {
    let res = match event_type {
        EventType::KeyPress(key) => {
            let code = code_from_key(*key)?;
            xtest::XTestFakeKeyEvent(display, code, TRUE, 0)
        }
        EventType::KeyRelease(key) => {
            let code = code_from_key(*key)?;
            xtest::XTestFakeKeyEvent(display, code, FALSE, 0)
        }
        EventType::ButtonPress(button) => match button {
            Button::Left => xtest::XTestFakeButtonEvent(display, 1, TRUE, 0),
            Button::Middle => xtest::XTestFakeButtonEvent(display, 2, TRUE, 0),
            Button::Right => xtest::XTestFakeButtonEvent(display, 3, TRUE, 0),
            Button::Unknown(code) => {
                xtest::XTestFakeButtonEvent(display, (*code).try_into().ok()?, TRUE, 0)
            }
        },
        EventType::ButtonRelease(button) => match button {
            Button::Left => xtest::XTestFakeButtonEvent(display, 1, FALSE, 0),
            Button::Middle => xtest::XTestFakeButtonEvent(display, 2, FALSE, 0),
            Button::Right => xtest::XTestFakeButtonEvent(display, 3, FALSE, 0),
            Button::Unknown(code) => {
                xtest::XTestFakeButtonEvent(display, (*code).try_into().ok()?, FALSE, 0)
            }
        },
        EventType::MouseMove { x, y } => {
            //TODO: replace with clamp if it is stabalized
            let x = if x.is_finite() {
                x.min(c_int::max_value().into())
                    .max(c_int::min_value().into())
                    .round() as c_int
            } else {
                0
            };
            let y = if y.is_finite() {
                y.min(c_int::max_value().into())
                    .max(c_int::min_value().into())
                    .round() as c_int
            } else {
                0
            };
            xtest::XTestFakeMotionEvent(display, 0, x, y, 0)
            //     xlib::XWarpPointer(display, 0, root, 0, 0, 0, 0, *x as i32, *y as i32);
        }
        EventType::Wheel { delta_x, delta_y } => {
            let code_x = if *delta_x > 0 { 7 } else { 6 };
            let code_y = if *delta_y > 0 { 4 } else { 5 };

            let mut result: c_int = 1;
            for _ in 0..delta_x.abs() {
                result = result
                    & xtest::XTestFakeButtonEvent(display, code_x, TRUE, 0)
                    & xtest::XTestFakeButtonEvent(display, code_x, FALSE, 0)
            }
            for _ in 0..delta_y.abs() {
                result = result
                    & xtest::XTestFakeButtonEvent(display, code_y, TRUE, 0)
                    & xtest::XTestFakeButtonEvent(display, code_y, FALSE, 0)
            }
            result
        }
    };
    if res == 0 {
        None
    } else {
        Some(())
    }
}

pub fn simulate(event_type: &EventType) -> Result<(), SimulateError> {
    unsafe {
        let dpy = xlib::XOpenDisplay(null());
        if dpy.is_null() {
            return Err(SimulateError);
        }
        match send_native(event_type, dpy) {
            Some(_) => {
                xlib::XFlush(dpy);
                xlib::XSync(dpy, 0);
                xlib::XCloseDisplay(dpy);
                Ok(())
            }
            None => {
                xlib::XCloseDisplay(dpy);
                Err(SimulateError)
            }
        }
    }
}