mod datachannel;
use std::{sync::{Arc}, time::{UNIX_EPOCH, SystemTime, self}, str::Split, thread, collections::HashMap};
use enigo::{Enigo, MouseControllable};
use rdev::{/* simulate,  */Button, EventType, Key as Key2, SimulateError};
//use webrtc::data_channel::RTCDataChannel;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use lazy_static::__Deref;
use crate::main_process::datachannel::{process_datachannel_messages, MouseOffset, PostSleepData};
use copypasta::{ClipboardContext, ClipboardProvider};

mod rdev_horizontal_wheel_fix;
use crate::main_process::rdev_horizontal_wheel_fix::{simulate};

const MOUSE_ROLLING_AVG_MULT : f64 = 0.025;
const MOUSE_TOO_SLOW : f64 = 1.05;
const MOUSE_TOO_FAST : f64 = 0.95;
const WHEEL_LINE_IN_PIXELS: f64 = 17.0; // DOM_DELTA_LINE in chromiun 2023, https://stackoverflow.com/a/37474225  
const ENIGO_MESSAGE_BUFFER_SIZE : usize = 250;
const CLOSE: &str = "CLOSE";

lazy_static! {
    static ref MOUSE_OFFSET_FROM_REAL: Arc<std::sync::Mutex<MouseOffset>> = Arc::new(std::sync::Mutex::new(MouseOffset { x: 0, y: 0 }));
    static ref MOUSE_LATEST_NANO: Arc<std::sync::Mutex<Option<u128>>> = Arc::new(std::sync::Mutex::new(None));
    static ref MOUSE_ROLLING_AVG_UPDATE_INTERVAL: Arc<std::sync::Mutex<u128>> = Arc::new(std::sync::Mutex::new(1000000000/60)); // Assume 60 updates/second at the start
    static ref WHEEL_SUB_PIXEL_X: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));
    static ref LINUX_WHEEL_SUB_PIXEL_X: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));
    static ref WHEEL_SUB_PIXEL_Y: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));
    static ref KEYBOARD_ALTGR_PRESSED: Arc<std::sync::Mutex<bool>> = Arc::new(std::sync::Mutex::new(false));
}

fn get_epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn send(event_type: &EventType) {
    //let delay = time::Duration::from_millis(20);
    match simulate(event_type) {
        Ok(()) => (),
        Err(SimulateError) => {
            println!("We could not send {:?}", event_type);
        }
    }
    // Let ths OS catchup (at least MacOS)
    //if cfg!(linux) {
        //println!("LINUX");
        //thread::sleep(delay);
    //}
}

fn handle_mousemove(mut values: Split<&str>, mut post_sleep_data: PostSleepData, enigo_handler_tx: SyncSender<String>) -> (Option<u128>, PostSleepData) {
    // Move immediately to new position. Take mouse offset into account
    // (this point may've been forecasted before)
    // Sleep before next forecast, unless lagging:
    // When lagging theres is extra gap (slow) and then burst of positions (fast)

    // Get the next real position (relative)
    let x = values.next().unwrap().parse::<i32>().unwrap();
    let y = values.next().unwrap().parse::<i32>().unwrap();

    // Calculate the how much should be moved when
    //forecast position has been taken into account
    // Also have the next forecast position (half of the real relative movement)
    let offset_x;
    let offset_y;
    {
        let mut mouse_offset = MOUSE_OFFSET_FROM_REAL.lock().unwrap();
        offset_x = x - mouse_offset.x;
        offset_y = y - mouse_offset.y;
        mouse_offset.x = x / 2;
        mouse_offset.y = y / 2;
        post_sleep_data.mouse_offset.x = x / 2;
        post_sleep_data.mouse_offset.y = y / 2;
    }

    // Move mouse
    let command = format!("mouse_move_relative,{},{}", offset_x, offset_y);
    match enigo_handler_tx.send(command) {
        Ok(_) => (),
        Err(e) => println!("Could not send Enigo close: {}", e),
    }
    //enigo.mouse_move_relative(offset_x, offset_y);

    // Update latest mouse nano and save the difference to the previous
    let now = get_epoch_nanos();
    let diff;
    {
        let mut mouse_latest_nano_ref = MOUSE_LATEST_NANO.lock().unwrap();
        let mouse_latest_nano = mouse_latest_nano_ref.deref();
        
        diff = match mouse_latest_nano {
            Some(mouse_latest_nano) => Some(now - mouse_latest_nano),
            None => None,
        };
        *mouse_latest_nano_ref = Some(now);
    }

    // Save half of the difference as sleep time if there has not been lags
    // OR the diff is None, which mean this has been the first move after "mouseidle"
    let sleep_amount = match diff {
        Some(diff) => {
            let mut mouse_rolling_avg_interval_ref = MOUSE_ROLLING_AVG_UPDATE_INTERVAL.lock().unwrap();
            *mouse_rolling_avg_interval_ref = ((*mouse_rolling_avg_interval_ref as f64) * (1.0 - MOUSE_ROLLING_AVG_MULT) + (diff as f64) * MOUSE_ROLLING_AVG_MULT) as i64 as u128;
            //println!("diff: {}", mouse_rolling_avg_interval_ref);
            let diff64: u64 = diff.try_into().unwrap();
            let value = diff64 as f64 / *mouse_rolling_avg_interval_ref as f64;
        
            if value > MOUSE_TOO_SLOW {
                println!("TOO SLOW: {}, diff: {}", value, mouse_rolling_avg_interval_ref);
                {
                    let mut mouse_offset = MOUSE_OFFSET_FROM_REAL.lock().unwrap();
                    mouse_offset.x = 0;
                    mouse_offset.y = 0;
                }
                post_sleep_data.mouse_offset.x = 0;
                post_sleep_data.mouse_offset.y = 0;
                None
            } else if value < MOUSE_TOO_FAST {
                println!("TOO FAST: {}, diff: {}", value, mouse_rolling_avg_interval_ref);
                {
                    let mut mouse_offset = MOUSE_OFFSET_FROM_REAL.lock().unwrap();
                    mouse_offset.x = 0;
                    mouse_offset.y = 0;
                }
                post_sleep_data.mouse_offset.x = 0;
                post_sleep_data.mouse_offset.y = 0;
                None
            } else {
                //println!("GOOD: {}, diff: {}", value, mouse_rolling_avg_interval_ref);
                Some(*mouse_rolling_avg_interval_ref / 2)
            }
            
        },
        None => {
            let mouse_rolling_avg_interval_ref = MOUSE_ROLLING_AVG_UPDATE_INTERVAL.lock().unwrap();
            Some(*mouse_rolling_avg_interval_ref / 2)
        },
    };

    return (sleep_amount, post_sleep_data);
}

fn handle_mouseidle () {
    // Reset mouse latest nano time
    // This will make average mouse update interval more accurate
    // Also reset the offset (keep the "wrong"/forecasted position at the end)
    //println!("mouseidle");
    {
        let mut mouse_latest_nano_ref = MOUSE_LATEST_NANO.lock().unwrap();
        *mouse_latest_nano_ref = None;
    }
    {
        let mut mouse_offset = MOUSE_OFFSET_FROM_REAL.lock().unwrap();
        mouse_offset.x = 0;
        mouse_offset.y = 0;
    }
}

fn handle_mousedown(mut values: Split<&str>/* , enigo_handler_tx: SyncSender<String> */) {
    let button = values.next().unwrap().parse::<i32>().unwrap();
    let command = format!("mouse_down,{}", button);
    println!("{}", command);

    // values from here: https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button#value
    //
    // On Linux (GTK), the 4th button and the 5th button are not supported. (Browser side, https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons#firefox_notes)
    let button = match button {
        0 => Button::Left,
        1 => Button::Middle,
        2 => Button::Right,
        // FORWARD BUTTON
        // From: https://github.com/enigo-rs/enigo/blob/de1828ab0a76f193eaab4b75aa76044377810e4a/src/win/win_impl.rs#L163
        #[cfg(target_os = "windows")]
        3 => Button::Unknown(2),
        // From: https://github.com/enigo-rs/enigo/blob/1df84701a7c239835e1962961411b3074676f5d4/src/linux.rs#L74
        #[cfg(target_os = "linux")]
        3 => Button::Unknown(9),
        // BACK BUTTON
        // From: https://github.com/enigo-rs/enigo/blob/de1828ab0a76f193eaab4b75aa76044377810e4a/src/win/win_impl.rs#L162
        #[cfg(target_os = "windows")]
        4 => Button::Unknown(1),
        // https://github.com/enigo-rs/enigo/blob/1df84701a7c239835e1962961411b3074676f5d4/src/linux.rs#L75
        #[cfg(target_os = "linux")]
        4 => Button::Unknown(8),
        _ => {
            println!("Unknown mouse button");
            return;
        },
    };
    send(&EventType::ButtonPress(button));
}

fn handle_mouseup(mut values: Split<&str>/* , enigo_handler_tx: SyncSender<String> */) {
    let button = values.next().unwrap().parse::<i32>().unwrap();
    let command = format!("mouse_up,{}", button);
    println!("{}", command);

    // values from here: https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button#value
    //
    // On Linux (GTK), the 4th button and the 5th button are not supported. (Browser side, https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons#firefox_notes)
    let button = match button {
        0 => Button::Left,
        1 => Button::Middle,
        2 => Button::Right,
        // FORWARD BUTTON
        // From: https://github.com/enigo-rs/enigo/blob/de1828ab0a76f193eaab4b75aa76044377810e4a/src/win/win_impl.rs#L163
        #[cfg(target_os = "windows")]
        3 => Button::Unknown(2),
        // From: https://github.com/enigo-rs/enigo/blob/1df84701a7c239835e1962961411b3074676f5d4/src/linux.rs#L74
        #[cfg(target_os = "linux")]
        3 => Button::Unknown(9),
        // BACK BUTTON
        // From: https://github.com/enigo-rs/enigo/blob/de1828ab0a76f193eaab4b75aa76044377810e4a/src/win/win_impl.rs#L162
        #[cfg(target_os = "windows")]
        4 => Button::Unknown(1),
        // https://github.com/enigo-rs/enigo/blob/1df84701a7c239835e1962961411b3074676f5d4/src/linux.rs#L75
        #[cfg(target_os = "linux")]
        4 => Button::Unknown(8),
        _ => {
            println!("Unknown mouse button");
            return;
        },
    };
    send(&EventType::ButtonRelease(button));
}

fn handle_wheel(mut values: Split<&str>) {
    let delta_mode = values.next().unwrap().parse::<i32>().unwrap();
    let mut x = values.next().unwrap().parse::<f64>().unwrap();
    let mut y = values.next().unwrap().parse::<f64>().unwrap();

    // deltaModes: https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event#event_properties
    // Treat DOM_DELTA_LINE and DOM_DELTA_PAGE the same for now
    if delta_mode != 0 {
        x *= WHEEL_LINE_IN_PIXELS;
        y *= WHEEL_LINE_IN_PIXELS;
    }

    let full_pixels_x;
    {
        let mut wheel_sub_pixel_ref = WHEEL_SUB_PIXEL_X.lock().unwrap();
        let combined = x + wheel_sub_pixel_ref.deref();
        full_pixels_x = (combined / 1.0) as i64;
        *wheel_sub_pixel_ref = combined % 1.0;
        //println!("reminder x {}", wheel_sub_pixel_ref.deref());
    }

    let full_pixels_y;
    {
        let mut wheel_sub_pixel_ref = WHEEL_SUB_PIXEL_Y.lock().unwrap();
        let combined = y + wheel_sub_pixel_ref.deref();
        full_pixels_y = (combined / 1.0) as i64;
        *wheel_sub_pixel_ref = combined % 1.0;
        //println!("reminder y {}", wheel_sub_pixel_ref.deref());
    }

    if full_pixels_x != 0 || full_pixels_y != 0 {
        println!("wheel x:{} y:{}", -full_pixels_x, -full_pixels_y);
        send(&EventType::Wheel {
            delta_x: full_pixels_x,
            delta_y: -full_pixels_y,
        });
    } else {
        println!("scroll more!");
    }
}

fn handle_keydown(mut values: Split<&str>) {
    // TODO make sutre there is at least 20 ms between kay presses (even on rdev)
    // https://github.com/enigo-rs/enigo/issues/105


    let code = values.next().unwrap();
    let key = values.next().unwrap();

    let rdev_code_to_key = HashMap::from([
        ("AltLeft", Key2::Alt),
        ("AltRight", Key2::AltGr),
        ("Backspace", Key2::Backspace),
        ("CapsLock", Key2::CapsLock),
        ("ControlLeft", Key2::ControlLeft),
        ("ControlRight", Key2::ControlRight),
        ("Delete", Key2::Delete),
        ("ArrowDown", Key2::DownArrow),
        ("End", Key2::End),
        ("Escape", Key2::Escape),
        ("F1", Key2::F1),
        ("F10", Key2::F10),
        ("F11", Key2::F11),
        ("F12", Key2::F12),
        ("F2", Key2::F2),
        ("F3", Key2::F3),
        ("F4", Key2::F4),
        ("F5", Key2::F5),
        ("F6", Key2::F6),
        ("F7", Key2::F7),
        ("F8", Key2::F8),
        ("F9", Key2::F9),
        ("Home", Key2::Home),
        ("ArrowLeft", Key2::LeftArrow),
        ("MetaLeft", Key2::MetaLeft),
        ("OSLeft", Key2::MetaLeft),
        ("MetaRight", Key2::MetaRight),
        ("OSRight", Key2::MetaRight),
        ("PageDown", Key2::PageDown),
        ("PageUp", Key2::PageUp),
        ("Enter", Key2::Return),
        ("ArrowRight", Key2::RightArrow),
        ("ShiftLeft", Key2::ShiftLeft),
        ("ShiftRight", Key2::ShiftRight),
        ("Space", Key2::Space),
        ("Tab", Key2::Tab),
        ("ArrowUp", Key2::UpArrow),
        ("PrintScreen", Key2::PrintScreen),
        ("ScrollLock", Key2::ScrollLock),
        ("Pause", Key2::Pause),
        ("NumLock", Key2::NumLock),
        ("Backquote", Key2::BackQuote),
        ("Digit1", Key2::Num1),
        ("Digit2", Key2::Num2),
        ("Digit3", Key2::Num3),
        ("Digit4", Key2::Num4),
        ("Digit5", Key2::Num5),
        ("Digit6", Key2::Num6),
        ("Digit7", Key2::Num7),
        ("Digit8", Key2::Num8),
        ("Digit9", Key2::Num9),
        ("Digit0", Key2::Num0),
        ("Minus", Key2::Minus),
        ("Equal", Key2::Equal),
        ("KeyQ", Key2::KeyQ),
        ("KeyW", Key2::KeyW),
        ("KeyE", Key2::KeyE),
        ("KeyR", Key2::KeyR),
        ("KeyT", Key2::KeyT),
        ("KeyY", Key2::KeyY),
        ("KeyU", Key2::KeyU),
        ("KeyI", Key2::KeyI),
        ("KeyO", Key2::KeyO),
        ("KeyP", Key2::KeyP),
        ("BracketLeft", Key2::LeftBracket),
        ("BracketRight", Key2::RightBracket),
        ("KeyA", Key2::KeyA),
        ("KeyS", Key2::KeyS),
        ("KeyD", Key2::KeyD),
        ("KeyF", Key2::KeyF),
        ("KeyG", Key2::KeyG),
        ("KeyH", Key2::KeyH),
        ("KeyJ", Key2::KeyJ),
        ("KeyK", Key2::KeyK),
        ("KeyL", Key2::KeyL),
        ("Semicolon", Key2::SemiColon),
        ("Quote", Key2::Quote),
        ("Backslash", Key2::BackSlash),
        ("IntlBackslash", Key2::IntlBackslash),
        ("KeyZ", Key2::KeyZ),
        ("KeyX", Key2::KeyX),
        ("KeyC", Key2::KeyC),
        ("KeyV", Key2::KeyV),
        ("KeyB", Key2::KeyB),
        ("KeyN", Key2::KeyN),
        ("KeyM", Key2::KeyM),
        ("Comma", Key2::Comma),
        ("Period", Key2::Dot),
        ("Slash", Key2::Slash),
        ("Insert", Key2::Insert),
        /* ("", Key2::KpReturn),
        ("", Key2::KpMinus),
        ("", Key2::KpPlus),
        ("", Key2::KpMultiply),
        ("", Key2::KpDivide),
        ("", Key2::Kp0),
        ("", Key2::Kp1),
        ("", Key2::Kp2),
        ("", Key2::Kp3),
        ("", Key2::Kp4),
        ("", Key2::Kp5),
        ("", Key2::Kp6),
        ("", Key2::Kp7),
        ("", Key2::Kp8),
        ("", Key2::Kp9),
        ("", Key2::KpDelete), */
        ("Fn", Key2::Function), // Frontend does not fire this event actually, unless maybe on Firefox Android?
    ]);



    let command = format!("key_down,{},{}", code, key);
    println!("{}", command);

    // At least Windows fires extra ControlLeft with AltGr event
    // https://bugzilla.mozilla.org/show_bug.cgi?id=900750
    // If not this, @ would not work
    if code == "AltRight" {
        println!("RELEASING ControlLeft");
        send(&EventType::KeyRelease(Key2::ControlLeft));
    }

    let key = rdev_code_to_key.get(code);
    match key {
        Some(key) => send(&EventType::KeyPress(*key)),
        None => println!("Unknown code: {}", code),
    }
}

fn handle_keyup(mut values: Split<&str>) {
    // https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_code_values
    // https://source.chromium.org/chromium/chromium/src/+/main:ui/events/keycodes/dom/dom_code_data.inc;l=344;drc=3344b61f7c7f06cf96069751c3bd64d8ec3e3428
    let rdev_code_to_key = HashMap::from([
        ("AltLeft", Key2::Alt),
        ("AltRight", Key2::AltGr),
        ("Backspace", Key2::Backspace),
        ("CapsLock", Key2::CapsLock),
        ("ControlLeft", Key2::ControlLeft),
        ("ControlRight", Key2::ControlRight),
        ("Delete", Key2::Delete),
        ("ArrowDown", Key2::DownArrow),
        ("End", Key2::End),
        ("Escape", Key2::Escape),
        ("F1", Key2::F1),
        ("F10", Key2::F10),
        ("F11", Key2::F11),
        ("F12", Key2::F12),
        ("F2", Key2::F2),
        ("F3", Key2::F3),
        ("F4", Key2::F4),
        ("F5", Key2::F5),
        ("F6", Key2::F6),
        ("F7", Key2::F7),
        ("F8", Key2::F8),
        ("F9", Key2::F9),
        ("Home", Key2::Home),
        ("ArrowLeft", Key2::LeftArrow),
        ("MetaLeft", Key2::MetaLeft),
        ("OSLeft", Key2::MetaLeft),
        ("MetaRight", Key2::MetaRight),
        ("OSRight", Key2::MetaRight),
        ("PageDown", Key2::PageDown),
        ("PageUp", Key2::PageUp),
        ("Enter", Key2::Return),
        ("ArrowRight", Key2::RightArrow),
        ("ShiftLeft", Key2::ShiftLeft),
        ("ShiftRight", Key2::ShiftRight),
        ("Space", Key2::Space),
        ("Tab", Key2::Tab),
        ("ArrowUp", Key2::UpArrow),
        ("PrintScreen", Key2::PrintScreen),
        ("ScrollLock", Key2::ScrollLock),
        ("Pause", Key2::Pause),
        ("NumLock", Key2::NumLock),
        ("Backquote", Key2::BackQuote),
        ("Digit1", Key2::Num1),
        ("Digit2", Key2::Num2),
        ("Digit3", Key2::Num3),
        ("Digit4", Key2::Num4),
        ("Digit5", Key2::Num5),
        ("Digit6", Key2::Num6),
        ("Digit7", Key2::Num7),
        ("Digit8", Key2::Num8),
        ("Digit9", Key2::Num9),
        ("Digit0", Key2::Num0),
        ("Minus", Key2::Minus),
        ("Equal", Key2::Equal),
        ("KeyQ", Key2::KeyQ),
        ("KeyW", Key2::KeyW),
        ("KeyE", Key2::KeyE),
        ("KeyR", Key2::KeyR),
        ("KeyT", Key2::KeyT),
        ("KeyY", Key2::KeyY),
        ("KeyU", Key2::KeyU),
        ("KeyI", Key2::KeyI),
        ("KeyO", Key2::KeyO),
        ("KeyP", Key2::KeyP),
        ("BracketLeft", Key2::LeftBracket),
        ("BracketRight", Key2::RightBracket),
        ("KeyA", Key2::KeyA),
        ("KeyS", Key2::KeyS),
        ("KeyD", Key2::KeyD),
        ("KeyF", Key2::KeyF),
        ("KeyG", Key2::KeyG),
        ("KeyH", Key2::KeyH),
        ("KeyJ", Key2::KeyJ),
        ("KeyK", Key2::KeyK),
        ("KeyL", Key2::KeyL),
        ("Semicolon", Key2::SemiColon),
        ("Quote", Key2::Quote),
        ("Backslash", Key2::BackSlash),
        ("IntlBackslash", Key2::IntlBackslash),
        ("KeyZ", Key2::KeyZ),
        ("KeyX", Key2::KeyX),
        ("KeyC", Key2::KeyC),
        ("KeyV", Key2::KeyV),
        ("KeyB", Key2::KeyB),
        ("KeyN", Key2::KeyN),
        ("KeyM", Key2::KeyM),
        ("Comma", Key2::Comma),
        ("Period", Key2::Dot),
        ("Slash", Key2::Slash),
        ("Insert", Key2::Insert),
        /* ("", Key2::KpReturn),
        ("", Key2::KpMinus),
        ("", Key2::KpPlus),
        ("", Key2::KpMultiply),
        ("", Key2::KpDivide),
        ("", Key2::Kp0),
        ("", Key2::Kp1),
        ("", Key2::Kp2),
        ("", Key2::Kp3),
        ("", Key2::Kp4),
        ("", Key2::Kp5),
        ("", Key2::Kp6),
        ("", Key2::Kp7),
        ("", Key2::Kp8),
        ("", Key2::Kp9),
        ("", Key2::KpDelete), */
        ("Fn", Key2::Function), // Frontend does not fire this event actually, unless maybe on Firefox Android?
    ]);
    
    let code = values.next().unwrap();
    let key = values.next().unwrap();

    let command = format!("key_up,{},{}", code, key);
    println!("{}", command);
    
    let key = rdev_code_to_key.get(code);
    match key {
        Some(key) => send(&EventType::KeyRelease(*key)),
        None => println!("Unknown code: {}", code),
    }

}

fn handle_paste(mut values: Split<&str>) {
    let delay = time::Duration::from_millis(20);
    
    let data = values.next().unwrap();
    let mut ctx = ClipboardContext::new().unwrap();
    ctx.set_contents(data.to_owned()).unwrap();
    send(&EventType::KeyPress(Key2::ControlLeft));
    thread::sleep(delay);
    send(&EventType::KeyPress(Key2::KeyV));
    thread::sleep(delay);

    println!("Pasted {}!", data);
}

pub async fn main_process() {
    // Separate Enigo thread required on macOS: https://github.com/enigo-rs/enigo/issues/96#issuecomment-765253193
    let (enigo_handler_tx, rx) : (SyncSender<String>, Receiver<String>) = sync_channel(ENIGO_MESSAGE_BUFFER_SIZE);
    let enigo_handler = thread::spawn(move || {
        let mut enigo = Enigo::new();

        // TODO others here as well
        loop {
            match rx.recv() {
                Ok(message) => {
                    //println!("RECEIVED {}", message);
                    let mut values = message.split(",");
                    let name = values.next().unwrap().to_string();


                    if &name == CLOSE {
                        break;
                    } else if &name == "mouse_move_relative" {
                        let x = values.next().unwrap().parse::<i32>().unwrap();
                        let y = values.next().unwrap().parse::<i32>().unwrap();
                        enigo.mouse_move_relative(x, y);
                    } /* else if &name == "mouse_down" || &name == "mouse_up" {
                        // values fr

Simple library to listen and send events globally to keyboard and mouse on macOS, Windows and Linux (x11).

You can also check out Enigo which is another crate which helped me write this one.

This crate is so far a pet project for me to understand the Rom here: https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button#value
                        //
                        // On Linux (GTK), the 4th button and the 5th button are not supported. (Browser side, https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons#firefox_notes)
                        let button = match values.next().unwrap().parse::<i32>().unwrap() {
                            0 => Some(MouseButton::Left),
                            1 => Some(MouseButton::Middle),
                            2 => Some(MouseButton::Right),
                            #[cfg(any(target_os = "windows", target_os = "linux"))]
                            3 => Some(MouseButton::Back),
                            #[cfg(any(target_os = "windows", target_os = "linux"))]
                            4 => Some(MouseButton::Forward),
                            _ => {
                                println!("Unknown mouse button");
                                None
                            },
                        };
                        
                        if let Some(button) = button {
                            if &name == "mouse_down" {
                                println!("mouse down");
                                enigo.mouse_down(button);
                            } else {
                                println!("mouse up");
                                enigo.mouse_up(button);
                            }
                        }

                    }  */else {
                        println!("Unknown message.name: {}", name);
                    }
                    
                }
                Err(e) => {
                    println!("Enigo recv error {}:", e);
                    break;
                },
            }
        }
        println!("Enigo thread ended")
    });

    let on_message_immmediate = move |msg: String, enigo_handler_tx: SyncSender<String>| {
        let mut values = msg.split(",");
        let name = values.next().unwrap().to_string();

        let mut sleep_amount: Option<u128> = None;
        let mut post_sleep_data = PostSleepData {
            name: name.clone(),
            mouse_offset: MouseOffset {
                x: 0,
                y: 0
            },
        };

        if &name == "mousemove" {
            (sleep_amount, post_sleep_data) = handle_mousemove(values, post_sleep_data, enigo_handler_tx);
        } else if &name == "mouseidle" {
            handle_mouseidle();
        } else if &name == "mousedown" {
            handle_mousedown(values);
        } else if &name == "mouseup" {
            handle_mouseup(values);
        } else if &name == "wheel" {
            handle_wheel(values);
        } else if &name == "keydown" {
            handle_keydown(values);
        } else if &name == "keyup" {
            handle_keyup(values);
        } else if &name == "copy" || &name == "cut" {
            // give 50ms time for copy/cut before reading
            sleep_amount = Some(50 * 1000000);
        } else if &name == "paste" {
            handle_paste(values);
        } else {
            println!("Unknown event.name: {}", name);
        }

        return (sleep_amount, post_sleep_data);
    };
        
    let on_message_post_sleep = move |post_sleep_data: PostSleepData, enigo_handler_tx: SyncSender<String>| {
        if post_sleep_data.name == "mousemove"{
            // Move halfway halfway to the forecasted new position.
            // Will be taken into account on the next move.
            // Forecasts smoothen the operation, as the mouse updates are doubled.
            if post_sleep_data.mouse_offset.x == 0 && post_sleep_data.mouse_offset.y == 0 {
                println!("Zero move skipped");
                return;
            }

            let command = format!("mouse_move_relative,{},{}", post_sleep_data.mouse_offset.x, post_sleep_data.mouse_offset.y);
            match enigo_handler_tx.send(command) {
                Ok(_) => (),
                Err(e) => println!("Could not send Enigo close: {}", e),
            }
            //enigo.mouse_move_relative(post_sleep_data.mouse_offset.x, post_sleep_data.mouse_offset.y);
        }
    };

    process_datachannel_messages(enigo_handler_tx.clone(), on_message_immmediate, on_message_post_sleep).await;

    match enigo_handler_tx.send(CLOSE.to_string()) {
        Ok(_) => (),
        Err(e) => println!("Could not send Enigo close: {}", e),
    }

    enigo_handler.join().expect("Enigo thread has paniced");
}