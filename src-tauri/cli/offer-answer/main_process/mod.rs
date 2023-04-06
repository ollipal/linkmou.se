mod datachannel;
use std::{sync::{Arc}, time::{UNIX_EPOCH, SystemTime}};
use enigo::{Enigo, MouseControllable};
use lazy_static::__Deref;

use crate::main_process::datachannel::{process_datachannel_messages, MouseOffset, PostSleepData};

//const URL: &str = "ws://localhost:3001";
const MOUSE_ROLLING_AVG_MULT : f64 = 0.05;



fn get_epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

lazy_static! {
    static ref ENIGO: Arc<std::sync::Mutex<Option<Enigo>>> = Arc::new(std::sync::Mutex::new(None));
    static ref MOUSE_OFFSET_FROM_REAL: Arc<std::sync::Mutex<MouseOffset>> = Arc::new(std::sync::Mutex::new(MouseOffset { x: 0, y: 0 }));
    static ref MOUSE_LATEST_NANO: Arc<std::sync::Mutex<Option<u128>>> = Arc::new(std::sync::Mutex::new(None));
    static ref MOUSE_ROLLING_AVG_UPDATE_INTERVAL: Arc<std::sync::Mutex<u128>> = Arc::new(std::sync::Mutex::new(1000000000/60)); // Assume 60 updates/second at the start
}

pub async fn main_process() {
    {
        let mut enigo = ENIGO.lock().unwrap();
        *enigo = Some(Enigo::new());
    }

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
        };

        if name == "mousemove".to_string() { // .to_string()?
            let x = values.next().unwrap().parse::<i32>().unwrap();
            let y = values.next().unwrap().parse::<i32>().unwrap();

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

            {
                let mut enigo = ENIGO.lock().unwrap();
                enigo.as_mut().unwrap().mouse_move_relative(offset_x, offset_y);
            }

            let now = get_epoch_nanos();
            let diff;
            
            {
                let mut mouse_last_nano_ref = MOUSE_LATEST_NANO.lock().unwrap();
                let mouse_last_nano = mouse_last_nano_ref.deref();
                
                diff = match mouse_last_nano {
                    Some(mouse_last_nano) => Some(now - mouse_last_nano),
                    None => None,
                };
                *mouse_last_nano_ref = Some(now);
            }

            sleep_amount = match diff {
                Some(diff) => {
                    let mut mouse_rolling_avg_interval_ref = MOUSE_ROLLING_AVG_UPDATE_INTERVAL.lock().unwrap();
                    *mouse_rolling_avg_interval_ref = ((*mouse_rolling_avg_interval_ref as f64) * (1.0 - MOUSE_ROLLING_AVG_MULT) + (diff as f64) * MOUSE_ROLLING_AVG_MULT) as i64 as u128;
                    println!("diff: {}", mouse_rolling_avg_interval_ref);
                    let diff64: u64 = diff.try_into().unwrap();
                    let value = diff64 as f64 / *mouse_rolling_avg_interval_ref as f64;
                
                    if value > 1.15 {
                        println!("TOO SLOW: {}", value);
                        None
                    } else if value < 0.70 {
                        println!("TOO FAST: {}", value);
                        None
                    } else {
                        Some(*mouse_rolling_avg_interval_ref / 2)
                    }
                    
                },
                None => {
                    let mouse_rolling_avg_interval_ref = MOUSE_ROLLING_AVG_UPDATE_INTERVAL.lock().unwrap();
                    Some(*mouse_rolling_avg_interval_ref / 2)
                },
            };
        } else if name == "mouseidle".to_string() {
            println!("mouseidle");
            {
                let mut mouse_last_nano_ref = MOUSE_LATEST_NANO.lock().unwrap();
                *mouse_last_nano_ref = None;
            }
        } else {
            // NOTE
            println!("Unknown event.name: {}", name);
        }

        return (sleep_amount, post_sleep_data);
    };
        
    let on_message_post_sleep = move |post_sleep_data: PostSleepData| {
        if post_sleep_data.name == "mousemove"{
            {
                let mut enigo = ENIGO.lock().unwrap();
                enigo.as_mut().unwrap().mouse_move_relative(post_sleep_data.mouse_offset.x, post_sleep_data.mouse_offset.y);
            }
        }
    };

    process_datachannel_messages(on_message_immmediate, on_message_post_sleep).await;

}