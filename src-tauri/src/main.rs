// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use device_query::{DeviceQuery, DeviceState, MouseState, Keycode};
use enigo::*;
use rdev::{display_size}; // Use enigo main_display_size when it will be available: https://github.com/enigo-rs/enigo/pull/79
use tauri::State;
use std::{sync::Mutex};

struct Devices {
    enigo: Mutex<Enigo>,
}

#[tauri::command]
fn init(devices: State<Devices>) {
    *devices.enigo.lock().unwrap() = Enigo::new();
    println!("Rust: initialized");
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn mouse_move_relative(x: i32, y: i32, devices: State<Devices>) {
    //let device_state = DeviceState::new();
    //let mouse: MouseState = device_state.get_mouse();
    //println!("Current Mouse Coordinates: {:?}", mouse.coords);
    
    let mut enigo = devices.enigo.lock().unwrap();
    enigo.mouse_move_relative(x, y);
    //enigo.mouse_move_to(mouse.coords.0 + 10, mouse.coords.1 + 10);
}

fn main() {
    let (w, h) = display_size().unwrap();
    assert!(w > 0);
    assert!(h > 0);
    println!("Width: {} Height: {}", w, h);

    tauri::Builder::default()
        .manage(Devices { enigo: Default::default() })
        .invoke_handler(tauri::generate_handler![init, mouse_move_relative])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
