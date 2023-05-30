// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lazy_static;

use serde::Serialize;
use tauri::{App, Manager/* , CustomMenuItem, SystemTray, SystemTrayMenu */};
use tauri_plugin_positioner::{WindowExt, Position};
use std::sync::{mpsc::{channel}, Arc, Mutex};
use rdev::{end_rdev};
use rand::Rng;
use rand::rngs::OsRng;

mod main_process;
use crate::main_process::main_process;


/* use std::sync::Mutex; */
use std::{thread};

const ID_SECTION_LEN: i32 = 6;
const ID_SECTION_AMOUNT: i32 = 4;

fn random_lowercase_letter_or_digit() -> char {
    let mut rng = OsRng;
    // 01ol dropped due to being easy to mix
    //let chars: Vec<char> = "abcdefghijkmnpqrstuvwxyz23456789".chars().collect();
    let chars: Vec<char> = "a".chars().collect();
    let index = rng.gen_range(0..chars.len());
    chars.get(index).unwrap().clone()
}

fn random_id() -> String {
    let mut id = String::new();

    for _ in 0..ID_SECTION_AMOUNT {
        for _ in 0..ID_SECTION_LEN {
            id.push(random_lowercase_letter_or_digit());
        }
        id.push('-');
    }
    id.pop();

    return id;
}

lazy_static! {
    static ref RANDOM_ID: Arc<Mutex<String>> = Arc::new(Mutex::new(random_id()));
}

/* mod datachannel;
use crate::datachannel::create_data_channel;

mod background_loop;
use crate::background_loop::start_background_loop; */

#[derive(Clone, Serialize)]
struct MyEvent {
    name: String,
}

#[tauri::command]
fn get_random_id() -> String {
    return RANDOM_ID.lock().unwrap().to_string();
}

fn setup(app: &App) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    {
        let window = app.get_window("main").unwrap();
        window.open_devtools();
        window.close_devtools();
    }
    
    let win = app.get_window("main").unwrap();
    let _ = win.move_window(Position::BottomRight);
    let app_handle = app.handle();
    /* tauri::async_runtime::spawn(async move {
      // listen to the `event-name` (emitted on any window) */
      let id = app.listen_global("event-name", move |event| {
        println!("got event-name with payload {:?}", event.payload());
        app_handle.emit_all("my_event", MyEvent { name: "payload".to_string()}).unwrap();
      });
      // unlisten to the event using the `id` returned on the `listen_global` function
      // a `once_global` API is also exposed on the `App` struct
      /* app_handle.unlisten(id);

      // emit the `event-name` event to all webview windows on the frontend
      app_handle.emit_all("event-name", "Tauri is awesome!").unwrap();
    }); */
    //app_handle.emit_all("id", random_id()).unwrap();
    Ok(())
}

fn main() {
    //let (send_stop_1, recv_stop_1) = channel();
    let (send_stop_2, recv_stop_2) = channel();
    let (send_stop_3, recv_stop_3) = tokio::sync::mpsc::channel::<()>(1);
    //let (send_stop_4, recv_stop_4) = channel();
    let (send_finished, recv_finished) = channel();


    let _main_handler = thread::spawn(move || {
        let random_id = RANDOM_ID.lock().unwrap().to_string();
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                main_process(
                    random_id,
                    recv_stop_2,
                    recv_stop_3,
                    /* recv_stop_4, */
                    send_finished,
                ).await;
            });
    });

    /* let open = CustomMenuItem::new("open".to_string(), "Open");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    let tray_menu = SystemTrayMenu::new().add_item(open).add_item(quit);
    let tray = SystemTray::new().with_menu(tray_menu); */

    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        /* .system_tray(tray) */
        .setup(|app| setup(app))
        /* .manage(TauriState {
            enigo: Default::default(),
        }) */
        .invoke_handler(tauri::generate_handler![get_random_id])
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
