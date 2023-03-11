// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use enigo::*;
use rdev::display_size;
use rdev::{listen, Event};
use rdev::EventType::{MouseMove};
use tauri::App;
// Use enigo main_display_size when it will be available: https://github.com/enigo-rs/enigo/pull/79
use std::sync::Mutex;
use std::thread;
use tauri::Manager; // Required to access app in 'setup'
use tauri::State;

struct TauriState {
    enigo: Mutex<Enigo>,
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
    // Not entirely sure, but perhaps you could omit that error type
    let app_handle = app.handle();

    thread::spawn(move || {
        let emit_result =  app_handle.emit_all("system_event", "mouse_move!"); // Run this in a loop {} or whatever you want to do with the handle


        fn callback(event: Event) {
            println!("My callback {:?}", event);
            match event.event_type {
                //MouseMove { x, y}  => || { app_handle.emit_all("system_event", "mouse_move!") },
               _ => println!("something else"),
            }
        }

        // This will block.
        if let Err(error) = listen(callback) {
            println!("Error: {:?}", error)
        }
    });
    Ok(())
}

fn main() {
    let (w, h) = display_size().unwrap();
    assert!(w > 0);
    assert!(h > 0);
    println!("Width: {} Height: {}", w, h);

    tauri::Builder::default()
        .setup(|app| {
            setup(app)
        })
        .manage(TauriState {
            enigo: Default::default(),
        })
        .invoke_handler(tauri::generate_handler![init, mouse_move_relative])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
