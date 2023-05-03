mod datachannel;
use std::{sync::{Arc}, time::{UNIX_EPOCH, SystemTime, self}, str::Split, thread, collections::HashMap};
use rdev::{/* simulate,  */Button, EventType, Key as Key2, SimulateError};
//use webrtc::data_channel::RTCDataChannel;
use lazy_static::__Deref;
use crate::main_process::datachannel::{process_datachannel_messages, MouseOffset, PostSleepData};
use copypasta::{ClipboardContext, ClipboardProvider};
use rdev::display_size;
use rdev::EventType::{MouseMove};
use rdev::{listen, simulate, Event};

struct MousePosition {
    x: f64,
    y: f64,
}

struct MouseHasBeenCenter {
    top: bool,
    left: bool,
    right: bool,
    bottom: bool,
}

struct WindowSize {
    x: f64,
    y: f64,
}

const MOUSE_ROLLING_AVG_MULT : f64 = 0.025;
const MOUSE_TOO_SLOW : f64 = 1.05;
const MOUSE_TOO_FAST : f64 = 0.95;
const MOUSE_JUMP_DISTANCE: f64 = 5.0; // Distance from side when jumped
const MOUSE_CENTER_DISTANCE: f64 = 35.0; // Distance from side considered to have been "center"
const WHEEL_LINE_IN_PIXELS: f64 = 17.0; // DOM_DELTA_LINE in chromiun 2023, https://stackoverflow.com/a/37474225  

lazy_static! {
    static ref WINDOW_SIZE: Arc<std::sync::Mutex<WindowSize>> = Arc::new(std::sync::Mutex::new(WindowSize { x: 0.0, y: 0.0 }));
    static ref MOUSE_LATEST_POS: Arc<std::sync::Mutex<MousePosition>> = Arc::new(std::sync::Mutex::new(MousePosition { x: 0.0, y: 0.0 }));
    static ref MOUSE_OFFSET_FROM_REAL: Arc<std::sync::Mutex<MouseOffset>> = Arc::new(std::sync::Mutex::new(MouseOffset { x: 0, y: 0 }));
    static ref MOUSE_LATEST_NANO: Arc<std::sync::Mutex<Option<u128>>> = Arc::new(std::sync::Mutex::new(None));
    static ref MOUSE_ROLLING_AVG_UPDATE_INTERVAL: Arc<std::sync::Mutex<u128>> = Arc::new(std::sync::Mutex::new(1000000000/60)); // Assume 60 updates/second at the start
    static ref MOUSE_HAS_BEEN_CENTER: Arc<std::sync::Mutex<MouseHasBeenCenter>> = Arc::new(std::sync::Mutex::new(MouseHasBeenCenter { top: false, left: false, right: false, bottom: false }));
    static ref WHEEL_SUB_PIXEL_X: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));
    static ref WHEEL_SUB_PIXEL_Y: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));

    // https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_code_values
    // https://source.chromium.org/chromium/chromium/src/+/main:ui/events/keycodes/dom/dom_code_data.inc;l=344;drc=3344b61f7c7f06cf96069751c3bd64d8ec3e3428
    static ref CODE_TO_RDEV_KEY: HashMap<&'static str, Key2> = HashMap::from([
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
        ("NumpadEnter", Key2::KpReturn),
        ("NumpadSubtract", Key2::KpMinus),
        ("NumpadAdd", Key2::KpPlus),
        ("NumpadMultiply", Key2::KpMultiply),
        ("NumpadDivide", Key2::KpDivide),
        ("Numpad0", Key2::Kp0),
        ("Numpad1", Key2::Kp1),
        ("Numpad2", Key2::Kp2),
        ("Numpad3", Key2::Kp3),
        ("Numpad4", Key2::Kp4),
        ("Numpad5", Key2::Kp5),
        ("Numpad6", Key2::Kp6),
        ("Numpad7", Key2::Kp7),
        ("Numpad8", Key2::Kp8),
        ("Numpad9", Key2::Kp9),
        ("NumpadDecimal", Key2::KpDelete),
        ("Fn", Key2::Function), // Frontend does not fire this event actually, unless maybe on Firefox Android?
    ]);

    // values from here: https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button#value
    //
    // On Linux (GTK), the 4th button and the 5th button are not supported. (Browser side, https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/buttons#firefox_notes)
    static ref CODE_TO_RDEV_BUTTON: HashMap<i32, Button> = HashMap::from([
        (0, Button::Left),
        (1, Button::Middle),
        (2, Button::Right),
        // FORWARD BUTTON
        // From: https://github.com/enigo-rs/enigo/blob/de1828ab0a76f193eaab4b75aa76044377810e4a/src/win/win_impl.rs#L163
        #[cfg(target_os = "windows")]
        (3, Button::Unknown(2)),
        // From: https://github.com/enigo-rs/enigo/blob/1df84701a7c239835e1962961411b3074676f5d4/src/linux.rs#L74
        #[cfg(target_os = "linux")]
        (3, Button::Unknown(9)),
        // BACK BUTTON
        // From: https://github.com/enigo-rs/enigo/blob/de1828ab0a76f193eaab4b75aa76044377810e4a/src/win/win_impl.rs#L162
        #[cfg(target_os = "windows")]
        (4, Button::Unknown(1)),
        // https://github.com/enigo-rs/enigo/blob/1df84701a7c239835e1962961411b3074676f5d4/src/linux.rs#L75
        #[cfg(target_os = "linux")]
        (4, Button::Unknown(8)),
    ]);
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

fn update_window_size(x: f64, y: f64) {
    {
        let mut window_size = WINDOW_SIZE.lock().unwrap();
        window_size.x = x;
        window_size.y = y;
    }
}

fn update_mouse_position(x: f64, y: f64) {
    {
        let mut mouse_position = MOUSE_LATEST_POS.lock().unwrap();
        mouse_position.x = x;
        mouse_position.y = y;
    }
}

fn mouse_move_relative(delta_x: f64, delta_y: f64) -> (bool, f64) {
    let mut is_right = false;
    let mut side_position = 0.0;
    let (x, y);
    {
        let mouse_position = MOUSE_LATEST_POS.lock().unwrap();
        let window_size = WINDOW_SIZE.lock().unwrap();

        let new_x = mouse_position.x + delta_x;
        if new_x < 0.0 {
            x = 0.0;
        } else if new_x > window_size.x {
            x = window_size.x;
        } else {
            x = new_x;
        }

        let new_y = mouse_position.y + delta_y;
        if new_y < 0.0 {
            y = 0.0;
        } else if new_y > window_size.y {
            y = window_size.y;
        } else {
            y = new_y;
        }
    
        if x > window_size.x - 2.0 {
            println!("RELATIVE ScreenRight");
            {
                let mouse_has_been_center_ref = MOUSE_HAS_BEEN_CENTER.lock().unwrap();
                if mouse_has_been_center_ref.left {
                    is_right = true;
                    side_position = y / window_size.y;
                } else {
                    println!("Has not been center yet")
                }
            }

        } else if x < window_size.x - MOUSE_CENTER_DISTANCE {
            let mut mouse_has_been_center_ref = MOUSE_HAS_BEEN_CENTER.lock().unwrap();
            if !mouse_has_been_center_ref.left {
                mouse_has_been_center_ref.left = true;
                println!("Left Center")
            }
        }
    }
    send(&EventType::MouseMove { x, y });
    return (is_right, side_position);
}

fn handle_mousemove(mut values: Split<&str>, mut post_sleep_data: PostSleepData/* , enigo_handler_tx: SyncSender<String> */) -> (Option<u128>, PostSleepData) {
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
    let (is_right, side_position) = mouse_move_relative(offset_x.into(), offset_y.into());
    post_sleep_data.is_right = is_right;
    post_sleep_data.side_position = side_position;

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

    let button = match CODE_TO_RDEV_BUTTON.get(&button) {
        Some(button) => button,
        None => {
            println!("Unknown mouse button");
            return;
        },
    };
    send(&EventType::ButtonPress(*button));
}

fn handle_mouseup(mut values: Split<&str>) {
    let button = values.next().unwrap().parse::<i32>().unwrap();
    let command = format!("mouse_up,{}", button);
    println!("{}", command);
    
    let button = match CODE_TO_RDEV_BUTTON.get(&button) {
        Some(button) => button,
        None => {
            println!("Unknown mouse button");
            return;
        },
    };
    send(&EventType::ButtonRelease(*button));
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
        println!("wheel x:{} y:{}", full_pixels_x, -full_pixels_y);
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

    let command = format!("key_down,{},{}", code, key);
    println!("{}", command);

    // At least Windows fires extra ControlLeft with AltGr event
    // https://bugzilla.mozilla.org/show_bug.cgi?id=900750
    // If not this, @ would not work
    if code == "AltRight" {
        println!("RELEASING ControlLeft");
        send(&EventType::KeyRelease(Key2::ControlLeft));
    }

    let key = CODE_TO_RDEV_KEY.get(code);
    match key {
        Some(key) => send(&EventType::KeyPress(*key)),
        None => println!("Unknown code: {}", code),
    }
}

fn handle_keyup(mut values: Split<&str>) {
    let code = values.next().unwrap();
    let key = values.next().unwrap();

    let command = format!("key_up,{},{}", code, key);
    println!("{}", command);
    
    let key = CODE_TO_RDEV_KEY.get(code);
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

fn handle_leftjump(mut values: Split<&str>) {
    {
        let mut mouse_has_been_center_ref = MOUSE_HAS_BEEN_CENTER.lock().unwrap();
        mouse_has_been_center_ref.left = false;
    }
    let height = values.next().unwrap().parse::<f64>().unwrap();
    let window_size = WINDOW_SIZE.lock().unwrap();
    assert!(window_size.x >= MOUSE_JUMP_DISTANCE);   
    //println!("{:?}", EventType::MouseMove { x: window_size.x - MOUSE_JUMP_DISTANCE, y: window_size.y * height }); 
    send(&EventType::MouseMove { x: window_size.x - MOUSE_JUMP_DISTANCE, y: window_size.y * height });
}

fn handle_mousehide() {
    let window_size = WINDOW_SIZE.lock().unwrap();
    send(&EventType::MouseMove { x: window_size.x / 2.0 + 1.0, y: window_size.y });
}

pub async fn main_process() {
    let (display_width_u64, display_height_u64) = display_size().unwrap();
    assert!(display_width_u64 > 0);
    assert!(display_height_u64 > 0);
    let display_width = display_width_u64 as f64;
    let display_height = display_height_u64 as f64;
    println!("Width: {} Height: {}", display_width, display_height);

    update_window_size(display_width, display_height);
    update_mouse_position(display_width / 2.0, display_height / 2.0);

    // TODO somehow exit and end
    let rdev_listen_handle = thread::spawn(move || {
        let callback = move |event: Event| {
            match event.event_type {
                MouseMove { x, y } => {
                    update_mouse_position(x,y);
                    if x < 1.0 {
                        println!("ScreenLeft");
                    } else if x > display_width as f64 - 2.0 {
                        println!("ScreenRight");
                    } else if y < 1.0 {
                        // Top will be missed if left or right as well
                        println!("ScreenTop");
                    } else if y > display_height as f64 - 2.0 {
                        // Bottom will be missed if left or right as well
                        println!("ScreenBottom");
                    }
                }
                _ => (),
            }
            ()
        };

        // This will block.
        if let Err(error) = listen(callback) {
            println!("Error: {:?}", error)
        }
    });

    let on_message_immmediate = move |msg: String| {
        let mut values = msg.split(",");
        let name = values.next().unwrap().to_string();

        let mut sleep_amount: Option<u128> = None;
        let mut post_sleep_data = PostSleepData {
            name: name.clone(),
            mouse_offset: MouseOffset {
                x: 0,
                y: 0
            },
            is_right: false,
            side_position: 0.0,
        };

        if &name == "mousemove" {
            (sleep_amount, post_sleep_data) = handle_mousemove(values, post_sleep_data);
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
        } else if &name == "leftjump" {
            handle_leftjump(values);
        } else if &name == "mousehide" {
            handle_mousehide();
        } else {
            println!("Unknown event.name: {}", name);
        }

        return (sleep_amount, post_sleep_data);
    };
        
    let on_message_post_sleep = move |post_sleep_data: PostSleepData| {
        if post_sleep_data.name == "mousemove"{
            // Move halfway halfway to the forecasted new position.
            // Will be taken into account on the next move.
            // Forecasts smoothen the operation, as the mouse updates are doubled.
            if post_sleep_data.mouse_offset.x == 0 && post_sleep_data.mouse_offset.y == 0 {
                println!("Zero move skipped");
                return;
            }

            mouse_move_relative(post_sleep_data.mouse_offset.x.into(), post_sleep_data.mouse_offset.y.into());
        }
    };

    process_datachannel_messages(on_message_immmediate, on_message_post_sleep).await;
}