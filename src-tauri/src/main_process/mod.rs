mod datachannel;
use std::{sync::{Arc}, time::{UNIX_EPOCH, SystemTime, self}, str::Split, thread, collections::HashMap, panic};
use lazy_static::__Deref;
use crate::main_process::datachannel::{process_datachannel_messages, MouseOffset, PostSleepData};
use copypasta::{ClipboardContext, ClipboardProvider};
use rdev::{Button, EventType, Key, SimulateError, simulate, mouse_move_relative, scroll_lines, scroll_pixels};
use std::sync::mpsc::{Receiver, Sender};

struct MouseHasBeenCenter {
    top: bool,
    left: bool,
    right: bool,
    bottom: bool,
}

struct WindowSize {
    x: Option<i32>,
    y: Option<i32>,
}

struct MouseUpdateState {
    updates: i32,
    too_fasts: i32,
    too_slows: i32,
    last_update: u128,
}

const MOUSE_ROLLING_AVG_MULT : f64 = 0.025;
const MOUSE_TOO_SLOW : f64 = 1.05;
const MOUSE_TOO_FAST : f64 = 0.95;
const MOUSE_JUMP_DISTANCE: f64 = 5.0; // Distance from side when jumped
const MOUSE_CENTER_DISTANCE: i32 = 35; // Distance from side considered to have been "center"
const MOUSE_CHECK_FREQUENCY: i32 = 1000;
const MOUSE_TOO_FAST_UPDATES_LIMIT: u128 = 500000000;

const WHEEL_LINE_IN_PIXELS: f64 = 17.0; // DOM_DELTA_LINE in chromiun 2023, https://stackoverflow.com/a/37474225  
/* #[cfg(target_os = "windows")]
const WHEEL_SUPPORTS_PIXEL_MOVE: bool = true;
#[cfg(not(target_os = "windows"))]
const WHEEL_SUPPORTS_PIXEL_MOVE: bool = false; */

lazy_static! {
    static ref WINDOW_SIZE: Arc<std::sync::Mutex<WindowSize>> = Arc::new(std::sync::Mutex::new(WindowSize { x: None, y: None }));
    //static ref MOUSE_LATEST_POS: Arc<std::sync::Mutex<MousePosition>> = Arc::new(std::sync::Mutex::new(MousePosition { x: 0.0, y: 0.0 }));
    static ref MOUSE_OFFSET_FROM_REAL: Arc<std::sync::Mutex<MouseOffset>> = Arc::new(std::sync::Mutex::new(MouseOffset { x: 0, y: 0 }));
    static ref MOUSE_LATEST_NANO: Arc<std::sync::Mutex<Option<u128>>> = Arc::new(std::sync::Mutex::new(None));
    static ref MOUSE_ROLLING_AVG_UPDATE_INTERVAL: Arc<std::sync::Mutex<u128>> = Arc::new(std::sync::Mutex::new(1000000000/60)); // Assume 60 updates/second at the start
    static ref MOUSE_UPDATE_STATE: Arc<std::sync::Mutex<MouseUpdateState>> = Arc::new(std::sync::Mutex::new(MouseUpdateState { updates: 0, too_fasts: 0, too_slows: 0, last_update: 0 }));
    static ref MOUSE_HAS_BEEN_CENTER: Arc<std::sync::Mutex<MouseHasBeenCenter>> = Arc::new(std::sync::Mutex::new(MouseHasBeenCenter { top: false, left: false, right: false, bottom: false }));
    //static ref WHEEL_SUB_PIXEL_X: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));
    //static ref WHEEL_SUB_PIXEL_Y: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));
    static ref CHECK_SIDES: Arc<std::sync::Mutex<bool>> = Arc::new(std::sync::Mutex::new(false));

    // https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_code_values
    // https://source.chromium.org/chromium/chromium/src/+/main:ui/events/keycodes/dom/dom_code_data.inc;l=344;drc=3344b61f7c7f06cf96069751c3bd64d8ec3e3428
    static ref CODE_TO_RDEV_KEY: HashMap<&'static str, Key> = HashMap::from([
        ("AltLeft", Key::Alt),
        ("AltRight", Key::AltGr),
        ("Backspace", Key::Backspace),
        ("CapsLock", Key::CapsLock),
        ("ControlLeft", Key::ControlLeft),
        ("ControlRight", Key::ControlRight),
        ("Delete", Key::Delete),
        ("ArrowDown", Key::DownArrow),
        ("End", Key::End),
        ("Escape", Key::Escape),
        ("F1", Key::F1),
        ("F10", Key::F10),
        ("F11", Key::F11),
        ("F12", Key::F12),
        ("F2", Key::F2),
        ("F3", Key::F3),
        ("F4", Key::F4),
        ("F5", Key::F5),
        ("F6", Key::F6),
        ("F7", Key::F7),
        ("F8", Key::F8),
        ("F9", Key::F9),
        ("Home", Key::Home),
        ("ArrowLeft", Key::LeftArrow),
        ("MetaLeft", Key::MetaLeft),
        ("OSLeft", Key::MetaLeft),
        ("MetaRight", Key::MetaRight),
        ("OSRight", Key::MetaRight),
        ("PageDown", Key::PageDown),
        ("PageUp", Key::PageUp),
        ("Enter", Key::Return),
        ("ArrowRight", Key::RightArrow),
        ("ShiftLeft", Key::ShiftLeft),
        ("ShiftRight", Key::ShiftRight),
        ("Space", Key::Space),
        ("Tab", Key::Tab),
        ("ArrowUp", Key::UpArrow),
        ("PrintScreen", Key::PrintScreen),
        ("ScrollLock", Key::ScrollLock),
        ("Pause", Key::Pause),
        ("NumLock", Key::NumLock),
        ("Backquote", Key::BackQuote),
        ("Digit1", Key::Num1),
        ("Digit2", Key::Num2),
        ("Digit3", Key::Num3),
        ("Digit4", Key::Num4),
        ("Digit5", Key::Num5),
        ("Digit6", Key::Num6),
        ("Digit7", Key::Num7),
        ("Digit8", Key::Num8),
        ("Digit9", Key::Num9),
        ("Digit0", Key::Num0),
        ("Minus", Key::Minus),
        ("Equal", Key::Equal),
        ("KeyQ", Key::KeyQ),
        ("KeyW", Key::KeyW),
        ("KeyE", Key::KeyE),
        ("KeyR", Key::KeyR),
        ("KeyT", Key::KeyT),
        ("KeyY", Key::KeyY),
        ("KeyU", Key::KeyU),
        ("KeyI", Key::KeyI),
        ("KeyO", Key::KeyO),
        ("KeyP", Key::KeyP),
        ("BracketLeft", Key::LeftBracket),
        ("BracketRight", Key::RightBracket),
        ("KeyA", Key::KeyA),
        ("KeyS", Key::KeyS),
        ("KeyD", Key::KeyD),
        ("KeyF", Key::KeyF),
        ("KeyG", Key::KeyG),
        ("KeyH", Key::KeyH),
        ("KeyJ", Key::KeyJ),
        ("KeyK", Key::KeyK),
        ("KeyL", Key::KeyL),
        ("Semicolon", Key::SemiColon),
        ("Quote", Key::Quote),
        ("Backslash", Key::BackSlash),
        ("IntlBackslash", Key::IntlBackslash),
        ("KeyZ", Key::KeyZ),
        ("KeyX", Key::KeyX),
        ("KeyC", Key::KeyC),
        ("KeyV", Key::KeyV),
        ("KeyB", Key::KeyB),
        ("KeyN", Key::KeyN),
        ("KeyM", Key::KeyM),
        ("Comma", Key::Comma),
        ("Period", Key::Dot),
        ("Slash", Key::Slash),
        ("Insert", Key::Insert),
        ("NumpadEnter", Key::KpReturn),
        ("NumpadSubtract", Key::KpMinus),
        ("NumpadAdd", Key::KpPlus),
        ("NumpadMultiply", Key::KpMultiply),
        ("NumpadDivide", Key::KpDivide),
        ("Numpad0", Key::Kp0),
        ("Numpad1", Key::Kp1),
        ("Numpad2", Key::Kp2),
        ("Numpad3", Key::Kp3),
        ("Numpad4", Key::Kp4),
        ("Numpad5", Key::Kp5),
        ("Numpad6", Key::Kp6),
        ("Numpad7", Key::Kp7),
        ("Numpad8", Key::Kp8),
        ("Numpad9", Key::Kp9),
        ("NumpadDecimal", Key::KpDelete),
        ("Fn", Key::Function), // Frontend does not fire this event actually, unless maybe on Firefox Android?
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
    // Only if keyboard event!
    //if cfg!(linux) {
        //println!("LINUX");
        //thread::sleep(delay);
    //}
}

fn update_window_size(x: i32, y: i32) {
    {
        let mut window_size = WINDOW_SIZE.lock().unwrap();
        window_size.x = Some(x);
        window_size.y = Some(y);
    }
}

/* fn update_mouse_position(x: f64, y: f64) {
    {
        let mut mouse_position = MOUSE_LATEST_POS.lock().unwrap();
        mouse_position.x = x;
        mouse_position.y = y;
    }
} */

/* fn mouse_move_relative(delta_x: f64, delta_y: f64) /* -> (bool, f64)  */{
    //println!("moving {} {}",delta_x,delta_y);
    send(&EventType::MouseMoveRelative { x: delta_x, y: delta_y });

    let (x, y);
    {
        let mouse_position = MOUSE_LATEST_POS.lock().unwrap();
        x = mouse_position.x + delta_x;
        y = mouse_position.y + delta_y;
    }
    send(&EventType::MouseMove { x, y });
} */

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

    let check_sides = CHECK_SIDES.lock().unwrap().clone();

    // Move mouse
    let (start_x, start_y) = mouse_move_relative(offset_x, offset_y, check_sides);

    // Update if needs jumping
    if check_sides {
        {
            let window_size = WINDOW_SIZE.lock().unwrap();
            if start_x > window_size.x.unwrap_or(0) - 2 {
                println!("RELATIVE ScreenRight");
                {
                    let mouse_has_been_center_ref = MOUSE_HAS_BEEN_CENTER.lock().unwrap();
                    if mouse_has_been_center_ref.left {
                        post_sleep_data.is_right = true;
                        post_sleep_data.side_position = start_y as f64 / window_size.y.unwrap_or(start_y/2) as f64;
                        // TODO prevent mouse movement
                    } else {
                        println!("Has not been center yet")
                    }
                }

            } else if x < window_size.x.unwrap_or(0) - MOUSE_CENTER_DISTANCE {
                let mut mouse_has_been_center_ref = MOUSE_HAS_BEEN_CENTER.lock().unwrap();
                if !mouse_has_been_center_ref.left {
                    mouse_has_been_center_ref.left = true;
                    println!("Left Center")
                }
            }
        }
    }


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

            let mut mouse_update_state = MOUSE_UPDATE_STATE.lock().unwrap();
            mouse_update_state.updates += 1;

            if mouse_update_state.updates % MOUSE_CHECK_FREQUENCY == 0 {
                println!(
                    "Too fasts: {}, too slows: {}, avg: {}",
                    mouse_update_state.too_fasts,
                    mouse_update_state.too_slows,
                    mouse_rolling_avg_interval_ref,
                );
                mouse_update_state.updates = 0;
                mouse_update_state.too_fasts = 0;
                mouse_update_state.too_slows = 0;

                let now = get_epoch_nanos();
                let diff = now - mouse_update_state.last_update;
                if diff < MOUSE_TOO_FAST_UPDATES_LIMIT {
                    println!("TOO FAST ({})", diff);
                    post_sleep_data.is_too_fast = true;
                }
                //if mouse_update_state.last_update != 0 && mouse_update_state.last_update
                mouse_update_state.last_update = now;
            }
        
            if value > MOUSE_TOO_SLOW {
                //println!("TOO SLOW: {}, diff: {}", value, mouse_rolling_avg_interval_ref);
                mouse_update_state.too_slows += 1;
                {
                    let mut mouse_offset = MOUSE_OFFSET_FROM_REAL.lock().unwrap();
                    mouse_offset.x = 0;
                    mouse_offset.y = 0;
                }
                post_sleep_data.mouse_offset.x = 0;
                post_sleep_data.mouse_offset.y = 0;
                None
            } else if value < MOUSE_TOO_FAST {
                //println!("TOO FAST: {}, diff: {}", value, mouse_rolling_avg_interval_ref);
                mouse_update_state.too_fasts += 1;

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
    let x = values.next().unwrap().parse::<f64>().unwrap();
    let y = values.next().unwrap().parse::<f64>().unwrap();

    // deltaModes: https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event#event_properties
    // Treat DOM_DELTA_LINE and DOM_DELTA_PAGE the same for now
    match delta_mode {
        0 => scroll_pixels(x, y),
        _ => scroll_lines(x, y),
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
        send(&EventType::KeyRelease(Key::ControlLeft));
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
    send(&EventType::KeyPress(Key::ControlLeft));
    thread::sleep(delay);
    send(&EventType::KeyPress(Key::KeyV));
    thread::sleep(delay);

    println!("Pasted {}!", data);
}

fn handle_leftjump(mut values: Split<&str>) {
    {
        let mut mouse_has_been_center_ref = MOUSE_HAS_BEEN_CENTER.lock().unwrap();
        mouse_has_been_center_ref.left = false;
    }

    {
        let mut check_sides = CHECK_SIDES.lock().unwrap();
        *check_sides = true;
    }

    //let (mut prev_start_x, mut prev_start_y) = (-200, -200);
    //let (mut start_x, mut start_y) = (-100, -100);


    /* for delta in [/* 10000, 1000, 100,  */10/* , 1 */].iter() {
        println!("delta: {}", delta);
        loop {
            (start_x, start_y) = mouse_move_relative(*delta, 0, true);
            mouse_move_relative(-2, 0, true).1;
            if !(start_x == 0 && start_y == 0) && prev_start_x - 5 <= start_x && start_x <= prev_start_x + 5 {
                break;
            }
            println!("start_x: {}", start_x);
            prev_start_x = start_x;

            let delay = time::Duration::from_millis(200);
            thread::sleep(delay);
        }
    } */

    if WINDOW_SIZE.lock().unwrap().x.is_none() {
        // Some stupidly large values
        let triple_8k_x = 7680 * 3;
        let triple_8k_y = 4320 * 3;

        mouse_move_relative(triple_8k_x, 0, true);
        let delay = time::Duration::from_millis(20); // Give time to move
        thread::sleep(delay);
        let max_x = mouse_move_relative(1, 0, true).0;

        mouse_move_relative(0, triple_8k_y, true);
        let delay = time::Duration::from_millis(20); // Give time to move
        thread::sleep(delay);
        let max_y = mouse_move_relative(0, 1, true).1;

        println!("max_x!: {}", max_x);
        println!("max_y!: {}", max_y);

        update_window_size(max_x, max_y);
    }




    /* for delta in [/* 10000, 1000, 100,  */10/* , 1 */].iter() {
        println!("delta: {}", delta);
        loop {
            (start_x, start_y) = mouse_move_relative(0, *delta, true);
            mouse_move_relative(0, -2, true).1;
            if !(start_x == 0 && start_y == 0) && prev_start_y - 5 <= start_y && start_y <= prev_start_y + 5 {
                break;
            }
            println!("start_x: {}", start_y);
            prev_start_y = start_y;

            let delay = time::Duration::from_millis(200);
            thread::sleep(delay);
        }
    } */
    //let max_y = start_y;

    let height = values.next().unwrap().parse::<f64>().unwrap();
    //println!("{:?}", EventType::MouseMove { x: window_size.x - MOUSE_JUMP_DISTANCE, y: window_size.y * height }); 
    
    {
        let window_size = WINDOW_SIZE.lock().unwrap();
        send(&EventType::MouseMove { x: (window_size.x.unwrap_or(0) as f64) - MOUSE_JUMP_DISTANCE, y: (window_size.y.unwrap_or(0) as f64) * height });
    }
}

fn handle_mousehide() {
    let window_size = WINDOW_SIZE.lock().unwrap();
    send(&EventType::MouseMove { x: (window_size.x.unwrap_or(0) as f64), y: (window_size.y.unwrap_or(0) as f64) * 0.97 });
    {
        let mut mouse_has_been_center_ref = MOUSE_HAS_BEEN_CENTER.lock().unwrap();
        mouse_has_been_center_ref.left = false;
    }
    {
        let mut check_sides = CHECK_SIDES.lock().unwrap();
        *check_sides = false;
    }
}

pub async fn main_process(
    random_id: String,
    //recv_stop_1: Receiver<bool>,
    recv_stop_2: Receiver<bool>,
    recv_stop_3: tokio::sync::mpsc::Receiver<()>,
    /* recv_stop_4: Receiver<bool>, */
    send_finished: Sender<bool>,
) {
    // rdev::listen cannot be stopped, catching a panic is the only workaround
    // https://github.com/Narsil/rdev/issues/72#issuecomment-1374830094
    /* let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        if let Some(msg) = panic_info.payload().downcast_ref::<&str>() {
            if *msg == "LISTEN_STOP_PANIC" {
                // Handle the spesific panic
                return;
            }
        }

        // If the panic message doesn't match, use the default hook
        println!("Unknown panic caught");
        default_hook(panic_info);
    })); */

    /* let rdev_listen_handle = thread::spawn(move || {
        let callback = move |event: Event| {
            match event.event_type {
                MouseMove { x, y } => update_mouse_position(x,y),
                _ => (),
            };
            if let Ok(should_stop) = recv_stop_1.try_recv() {
                if should_stop {
                    println!("Listening stopped");
                    panic!("LISTEN_STOP_PANIC");
                }
            }
            ()
        };

        let result = panic::catch_unwind(|| {
            // This will block.
            if let Err(error) = listen(callback) {
                println!("Error: {:?}", error)
            }
        });

        match result {
            Ok(res) => res,
            Err(_) => println!("caught panic as expected!"),
        }
    }); */

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
            is_too_fast: false,
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
                //println!("Zero move skipped");
                return;
            }

            mouse_move_relative(post_sleep_data.mouse_offset.x, post_sleep_data.mouse_offset.y, false);
        }
    };

    process_datachannel_messages(
        random_id,
        on_message_immmediate,
        on_message_post_sleep,
        recv_stop_2,
        recv_stop_3,
    ).await;

    /* println!("Waiting listen to join");
    if let Err(_e) = rdev_listen_handle.join() {
        println!("Join thread err");
    } */

    if let Err(_e) = send_finished.send(true) {
        println!("Could not send finished");
    }
}