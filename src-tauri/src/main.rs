// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lazy_static;

use serde::Serialize;
use tauri::{App, CustomMenuItem, SystemTray, SystemTrayMenu};


mod main_process;
use crate::main_process::main_process;


/* use std::sync::Mutex; */
use std::thread;

/* mod datachannel;
use crate::datachannel::create_data_channel;

mod background_loop;
use crate::background_loop::start_background_loop; */

#[derive(Clone, Serialize)]
struct SystemEvent {
    name: String,
    x: u64,
    y: u64,
    // TODO screen width, height, this logic to front end side
}

#[tauri::command]
fn init(/* devices: State<TauriState> */) {
    /* *devices.enigo.lock().unwrap() = Enigo::new();
    println!("Rust: initialized"); */
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn mouse_move_relative(_x: i32,_y: i32, /* devices: State<TauriState> */) {
    /* let mut enigo = devices.enigo.lock().unwrap();
    enigo.mouse_move_relative(x, y); */
}

fn setup(app: &App) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let app_handle = app.handle();
    Ok(())
}

#[tokio::main]
async fn main() {
    let main_handler = thread::spawn(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                main_process().await;
            });
    });
    let open = CustomMenuItem::new("open".to_string(), "Open");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    let tray_menu = SystemTrayMenu::new().add_item(open).add_item(quit);
    let tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(tray)
        .setup(|app| setup(app))
        /* .manage(TauriState {
            enigo: Default::default(),
        }) */
        .invoke_handler(tauri::generate_handler![init, mouse_move_relative])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| match event {
            /* tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            } */
            _ => {}
        });
    
    println!("Waiting for main handler to join");
    main_handler.join().expect("Couldn't join main handler");
}
