// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use device_query::{DeviceQuery, DeviceState, MouseState, Keycode};
use enigo::*;
use rdev::{display_size};


// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    let device_state = DeviceState::new();
    let mouse: MouseState = device_state.get_mouse();
    println!("Current Mouse Coordinates: {:?}", mouse.coords);
    
    let mut enigo = Enigo::new();
    //enigo.mouse_move_relative(x, y) available as well
    enigo.mouse_move_to(mouse.coords.0 + 10, mouse.coords.1 + 10);

    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn main() {
    let (w, h) = display_size().unwrap(); // Use enigo main_display_size when it will be available: https://github.com/enigo-rs/enigo/pull/79
    assert!(w > 0);
    assert!(h > 0);
    println!("Width: {} Height: {}", w, h);

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
