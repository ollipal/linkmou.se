mod datachannel;
use std::{sync::{Arc}, time::{UNIX_EPOCH, SystemTime}, str::Split, thread, collections::HashMap};
use enigo::{Enigo, MouseControllable, MouseButton, Key, KeyboardControllable};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use lazy_static::__Deref;
use crate::main_process::datachannel::{process_datachannel_messages, MouseOffset, PostSleepData};

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
    static ref WHEEL_SUB_LINE_X: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));
    static ref WHEEL_SUB_LINE_Y: Arc<std::sync::Mutex<f64>> = Arc::new(std::sync::Mutex::new(0.0));
    static ref KEYBOARD_ALTGR_PRESSED: Arc<std::sync::Mutex<bool>> = Arc::new(std::sync::Mutex::new(false));
}

fn get_epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
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

fn handle_mousedown(mut values: Split<&str>, enigo_handler_tx: SyncSender<String>) {
    let button = values.next().unwrap().parse::<i32>().unwrap();
    let command = format!("mouse_down,{}", button);
    match enigo_handler_tx.send(command) {
        Ok(_) => (),
        Err(e) => println!("Could not send Enigo close: {}", e),
    }
}

fn handle_mouseup(mut values: Split<&str>, enigo_handler_tx: SyncSender<String>) {
    let button = values.next().unwrap().parse::<i32>().unwrap();
    let command = format!("mouse_up,{}", button);
    match enigo_handler_tx.send(command) {
        Ok(_) => (),
        Err(e) => println!("Could not send Enigo close: {}", e),
    }
}

fn handle_wheel(mut values: Split<&str>, enigo_handler_tx: SyncSender<String>) {
    let delta_mode = values.next().unwrap();
    let x = values.next().unwrap();
    let y = values.next().unwrap();
    if x != "0" {
        let command = format!("mouse_scroll_x,{},{}", delta_mode, x);
        match enigo_handler_tx.send(command) {
            Ok(_) => (),
            Err(e) => println!("Could not send Enigo close: {}", e),
        }
    }

    if y != "0" {
        let command = format!("mouse_scroll_y,{},{}", delta_mode, y);
        match enigo_handler_tx.send(command) {
            Ok(_) => (),
            Err(e) => println!("Could not send Enigo close: {}", e),
        }
    }
}

fn handle_keydown(mut values: Split<&str>, enigo_handler_tx: SyncSender<String>) {
    let code = values.next().unwrap();
    let key = values.next().unwrap();

    let command = format!("key_down,{},{}", code, key);
    match enigo_handler_tx.send(command) {
        Ok(_) => (),
        Err(e) => println!("Could not send Enigo close: {}", e),
    }
}

fn handle_keyup(mut values: Split<&str>, enigo_handler_tx: SyncSender<String>) {
    let code = values.next().unwrap();
    let key = values.next().unwrap();

    let command = format!("key_up,{},{}", code, key);
    match enigo_handler_tx.send(command) {
        Ok(_) => (),
        Err(e) => println!("Could not send Enigo close: {}", e),
    }
}

pub async fn main_process() {
    // Separate Enigo thread required on macOS: https://github.com/enigo-rs/enigo/issues/96#issuecomment-765253193
    let (enigo_handler_tx, rx) : (SyncSender<String>, Receiver<String>) = sync_channel(ENIGO_MESSAGE_BUFFER_SIZE);
    let enigo_handler = thread::spawn(move || {
        let mut enigo = Enigo::new();
        // https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_code_values
        // https://source.chromium.org/chromium/chromium/src/+/main:ui/events/keycodes/dom/dom_code_data.inc;l=344;drc=3344b61f7c7f06cf96069751c3bd64d8ec3e3428
        let code_to_key = HashMap::from([
            ("AltLeft", Key::Alt),
            //("AltRight", Key::Alt or Key::Commend), // This is handled separately with KEYBOARD_ALTGR_PRESSED
            //("", Key::Begin),
            //("", Key::Break),
            //("", Key::Cancel),
            ("CapsLock", Key::CapsLock),
            //("", Key::Clear),
            //("", Key::Command),
            //("", Key::Control),
            ("Delete", Key::Delete),
            ("ArrowDown", Key::DownArrow),
            ("End", Key::End),
            ("Escape", Key::Escape), // will never trigger probably, as the frontend stops controlling?
            //("", Key::Execute),
            // Some of the F not in use
            ("F1", Key::F1),
            ("F2", Key::F2),
            ("F3", Key::F3),
            ("F4", Key::F4),
            ("F5", Key::F5),
            ("F6", Key::F6),
            ("F7", Key::F7),
            ("F8", Key::F8),
            ("F9", Key::F9),
            ("F10", Key::F10),
            ("F11", Key::F11),
            ("F12", Key::F12),
            ("F13", Key::F13),
            ("F14", Key::F14),
            ("F15", Key::F15),
            ("F16", Key::F16),
            ("F17", Key::F17),
            ("F18", Key::F18),
            ("F19", Key::F19),
            ("F20", Key::F20),
            ("F21", Key::F21),
            ("F22", Key::F22),
            ("F23", Key::F23),
            ("F24", Key::F24),
            ("F25", Key::F25),
            ("F26", Key::F26),
            ("F27", Key::F27),
            ("F28", Key::F28),
            ("F29", Key::F29),
            ("F30", Key::F30),
            ("F31", Key::F31),
            ("F32", Key::F32),
            ("F33", Key::F33),
            ("F34", Key::F34),
            ("F35", Key::F35),
            ("Find", Key::Find),
            ("Lang1", Key::Hangul),
            ("Lang2", Key::Hanja), // Chromium only
            ("Help", Key::Help), // Gecko
            //("Insert", Key::Help),  // Chromium, HOW CAN BE ALSO "insert"?
            ("Home", Key::Home),
            ("Insert", Key::Insert),
            // Next three probably wrong, not sure
            ("Lang3", Key::Kanji), // Chromium only
            ("Lang4", Key::Kanji), // Chromium only
            ("Lang5", Key::Kanji), // Chromium only
            ("ControlLeft", Key::LControl),
            ("ArrowLeft", Key::LeftArrow),
            //("", Key::Linefeed),
            ("ContextMenu", Key::LMenu),
            ("ShiftLeft", Key::LShift),
            ("OSLeft", Key::Meta),
            ("MetaLeft", Key::Meta),
            //("", Key::ModeChange),
            ("NumLock", Key::Numlock),
            //("", Key::Option),
            ("PageDown", Key::PageDown),
            ("PageUp", Key::PageUp),
            ("Pause", Key::Pause),
            ("PrintScreen", Key::Print),
            ("ControlRight", Key::RControl),
            //("", Key::Redo),
            ("Enter", Key::Return),
            ("ArrowRight", Key::RightArrow),
            ("ShiftRight", Key::RShift),
            ("ScrollLock", Key::ScrollLock),
            ("Select", Key::Select),
            //("", Key::ScriptSwitch),
            //("", Key::Shift),
            //("", Key::ShiftLock),
            ("Space", Key::Space),
            //("", Key::Super),
            //("", Key::SysReq),
            ("Tab", Key::Tab),
            ("Undo", Key::Undo),
            ("ArrowUp", Key::UpArrow),
            //("", Key::Windows),
            ("Backspace", Key::Backspace),
        ]);

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
                    } else if &name == "mouse_down" || &name == "mouse_up" {
                        // values from here: https://developer.mozilla.org/en-US/docs/Web/API/MouseEvent/button#value
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

                    } else if &name == "mouse_scroll_x" {
                        let delta_mode = values.next().unwrap().parse::<i32>().unwrap();
                        let mut value = values.next().unwrap().parse::<f64>().unwrap();
                        // deltaModes: https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event#event_properties
                        // Treat DOM_DELTA_LINE and DOM_DELTA_PAGE the same for now
                        // THIS MIGHT HAVE SOME ROUNDING ISSUES
                        if delta_mode != 0 {
                            value *= WHEEL_LINE_IN_PIXELS;
                        }
                        let lines;
                        {
                            let mut wheel_sub_line_ref = WHEEL_SUB_LINE_X.lock().unwrap();
                            let combined = value + wheel_sub_line_ref.deref();
                            lines = (combined / WHEEL_LINE_IN_PIXELS) as i32;
                            *wheel_sub_line_ref = combined % WHEEL_LINE_IN_PIXELS;
                        }
                        if lines != 0 {
                            enigo.mouse_scroll_x(lines);
                        } else {
                            println!("scroll more!");
                        }                    
                    } else if &name == "mouse_scroll_y" {
                        let delta_mode = values.next().unwrap().parse::<i32>().unwrap();
                        let mut value = values.next().unwrap().parse::<f64>().unwrap();
                        // deltaModes: https://developer.mozilla.org/en-US/docs/Web/API/Element/wheel_event#event_properties
                        // Treat DOM_DELTA_LINE and DOM_DELTA_PAGE the same for now
                        // THIS MIGHT HAVE SOME ROUNDING ISSUES
                        if delta_mode != 0 {
                            value *= WHEEL_LINE_IN_PIXELS;
                        }
                        let lines;
                        {
                            let mut wheel_sub_line_ref = WHEEL_SUB_LINE_Y.lock().unwrap();
                            let combined = value + wheel_sub_line_ref.deref();
                            lines = (combined / WHEEL_LINE_IN_PIXELS) as i32;
                            *wheel_sub_line_ref = combined % WHEEL_LINE_IN_PIXELS;
                        }
                        if lines != 0 {
                            enigo.mouse_scroll_y(lines);
                        } else {
                            println!("scroll more!");
                        }
                    } else if &name == "key_down" || &name == "key_up" {
                        // TODO handle comma separately!
                        let code = values.next().unwrap();
                        let key_str = values.next().unwrap();
                        let key_char = key_str.chars().nth(0);

                        /* if code == "ControlLeft" {
                            println!("ControlLeft skipped");
                            continue;
                        } */

                        if code == "AltRight" {
                            {
                                let mut altgr_pressed_ref = KEYBOARD_ALTGR_PRESSED.lock().unwrap();
                                if &name == "key_down" {
                                    println!("Altgr pressed");
                                    *altgr_pressed_ref = true;
                                } else {
                                    println!("Altgr released");
                                    *altgr_pressed_ref = false;
                                }
                            }
                            continue;
                        }

                        let key;
                        if let Some(k) = code_to_key.get(code) {
                            key = Some(*k);
                        } else if let Some(key_char) = key_char {
                            key = Some(Key::Layout(key_char));
                        } else {
                            println!("Unknown key");
                            key = None;
                        }

                        if let Some(key) = key {
                            if &name == "key_down" {
                                {
                                    let altgr_pressed = KEYBOARD_ALTGR_PRESSED.lock().unwrap();
                                    if *altgr_pressed.deref() && key_str.len() == 1 {
                                        println!("{} sequence", key_str);
                                        enigo.key_up(Key::LControl);
                                        enigo.key_sequence(key_str);
                                        // TODO key down LContorl?
                                    } else {
                                        println!("{:?} down", key);
                                        enigo.key_down(key);
                                    }
                                }
                                //println!("({})", key_str);
                                //enigo.key_down(key);
                                //enigo.key_sequence(key_str);
                            } else {
                                println!("{:?} up", key);
                                enigo.key_up(key);
                            }
                        }
                        


                    } else {
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
            handle_mousedown(values, enigo_handler_tx);
        } else if &name == "mouseup" {
            handle_mouseup(values, enigo_handler_tx);
        } else if &name == "wheel" {
            handle_wheel(values, enigo_handler_tx);
        } else if &name == "keydown" {
            handle_keydown(values, enigo_handler_tx);
        } else if &name == "keyup" {
            handle_keyup(values, enigo_handler_tx);
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