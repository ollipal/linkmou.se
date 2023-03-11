// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use enigo::*;
use rdev::display_size;
use rdev::EventType::MouseMove;
use rdev::{listen, Event};
use serde::Serialize;
use tauri::{App, AppHandle};
// Use enigo main_display_size when it will be available: https://github.com/enigo-rs/enigo/pull/79
use std::sync::Mutex;
use std::thread;
use tauri::Manager; // Required to access app in 'setup'
use tauri::State;

struct TauriState {
    enigo: Mutex<Enigo>,
}

#[derive(Clone, Serialize)]
struct SystemEvent {
    name: String,
    x: u64,
    y: u64,
}

#[tauri::command]
fn init(devices: State<TauriState>) {
    *devices.enigo.lock().unwrap() = Enigo::new();
    println!("Rust: initialized");
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn mouse_move_relative(x: i32, y: i32, devices: State<TauriState>) {
    let mut enigo = devices.enigo.lock().unwrap();
    enigo.mouse_move_relative(x, y);
}

fn setup(app: &App) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let app_handle = app.handle();
    let (display_width_u64, display_height_u64) = display_size().unwrap();
    assert!(display_width_u64 > 0);
    assert!(display_height_u64 > 0);
    let display_width = display_width_u64 as f64;
    let display_height = display_height_u64 as f64;
    println!("Width: {} Height: {}", display_width, display_height);

    thread::spawn(move || {
        // Not sure why this did not work...
        /* fn send_system_event(app_handle: AppHandle, system_event: SystemEvent) -> std::option::Option<()> {
            app_handle
                .emit_all("system_event", system_event)
                .map_err(|err| println!("{:?}", err))
                .ok()
        } */

        let callback = move |event: Event| {
            match event.event_type {
                MouseMove { x, y } => {
                    if x < 1.0 {
                        app_handle
                            .emit_all(
                                "system_event",
                                SystemEvent {
                                    name: "ScreenLeft".to_string(),
                                    x: x as u64,
                                    y: y as u64,
                                },
                            )
                            .map_err(|err| println!("{:?}", err))
                            .ok();
                    } else if x > display_width as f64 - 2.0 {
                        app_handle
                            .emit_all(
                                "system_event",
                                SystemEvent {
                                    name: "ScreenRight".to_string(),
                                    x: x as u64,
                                    y: y as u64,
                                },
                            )
                            .map_err(|err| println!("{:?}", err))
                            .ok();
                    } else if y < 1.0 {
                        // Top will be missed if left or right as well
                        app_handle
                            .emit_all(
                                "system_event",
                                SystemEvent {
                                    name: "ScreenTop".to_string(),
                                    x: x as u64,
                                    y: y as u64,
                                },
                            )
                            .map_err(|err| println!("{:?}", err))
                            .ok();
                    } else if y > display_height as f64 - 2.0 {
                        // Bottom will be missed if left or right as well
                        app_handle
                            .emit_all(
                                "system_event",
                                SystemEvent {
                                    name: "ScreenBottom".to_string(),
                                    x: x as u64,
                                    y: y as u64,
                                },
                            )
                            .map_err(|err| println!("{:?}", err))
                            .ok();
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
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| setup(app))
        .manage(TauriState {
            enigo: Default::default(),
        })
        .invoke_handler(tauri::generate_handler![init, mouse_move_relative])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
