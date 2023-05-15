// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lazy_static;

use serde::Serialize;
use tauri::{App/* , CustomMenuItem, SystemTray, SystemTrayMenu */};
use std::sync::mpsc::{channel};
use rdev::{end_rdev};

mod main_process;
use crate::main_process::main_process;


/* use std::sync::Mutex; */
use std::{thread};

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

/* #[tauri::command]
fn init(/* devices: State<TauriState> */) {
    /* *devices.enigo.lock().unwrap() = Enigo::new();
    println!("Rust: initialized"); */
}

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn mouse_move_relative(_x: i32,_y: i32, /* devices: State<TauriState> */) {
    /* let mut enigo = devices.enigo.lock().unwrap();
    enigo.mouse_move_relative(x, y); */
} */

fn setup(app: &App) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    let _app_handle = app.handle();
    Ok(())
}

fn main() {
    //let (send_stop_1, recv_stop_1) = channel();
    let (send_stop_2, recv_stop_2) = channel();
    let (send_stop_3, recv_stop_3) = tokio::sync::mpsc::channel::<()>(1);
    //let (send_stop_4, recv_stop_4) = channel();
    let (send_finished, recv_finished) = channel();


    let _main_handler = thread::spawn(move || {
        //should_run.load(Ordering::Relaxed);
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                main_process(
                    recv_stop_2,
                    recv_stop_3,
                    /* recv_stop_4, */
                    send_finished
                ).await;
            });
    });

    /* let open = CustomMenuItem::new("open".to_string(), "Open");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    let tray_menu = SystemTrayMenu::new().add_item(open).add_item(quit);
    let tray = SystemTray::new().with_menu(tray_menu); */

    tauri::Builder::default()
        /* .system_tray(tray) */
        .setup(|app| setup(app))
        /* .manage(TauriState {
            enigo: Default::default(),
        }) */
        /* .invoke_handler(tauri::generate_handler![init, mouse_move_relative]) */
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { /* api, */ .. } => {
                /* api.prevent_exit(); */
                /* if let Err(e) = send_stop_1.send(true) {
                    println!("Could not send stop 1 {}", e);
                } */
                if let Err(e) = send_stop_2.send(true) {
                    println!("Could not send stop 2 {}", e);
                }
                if let Err(e) = send_stop_3.try_send(()) {
                    println!("Could not send stop 3 {}", e);
                }
                println!("Waiting for main_process to finish");
                let _res = recv_finished.recv(); // result value does not matter here
                
                end_rdev();
            }
            _ => {}
        });
    
    /* println!("----------------- Waiting for main handler to join");
    main_handler.join().expect("Couldn't join main handler"); */
}
