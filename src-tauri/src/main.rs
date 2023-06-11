// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate lazy_static;

use serde::Serialize;
use tauri::{App, Manager, AppHandle/* , CustomMenuItem, SystemTray, SystemTrayMenu */};
//use tauri_plugin_positioner::{WindowExt, Position};
use std::{sync::{mpsc::{channel}, Arc, Mutex}, thread::JoinHandle};
use rdev::{end_rdev};
use rand::Rng;
use rand::rngs::OsRng;

mod main_process;
use crate::main_process::main_process;


/* use std::sync::Mutex; */
use std::{thread};

const ID_SECTION_LEN: i32 = 6;
const ID_SECTION_AMOUNT: i32 = 1; // 4;

fn random_lowercase_letter_or_digit() -> char {
    let mut rng = OsRng;
    // 01ol dropped due to being easy to mix
    let chars: Vec<char> = "abcdefghijkmnpqrstuvwxyz23456789".chars().collect();
    //let chars: Vec<char> = "a".chars().collect();
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

struct StopInformation {
    send_stop_2: Option<std::sync::mpsc::Sender<bool>>,
    send_stop_3: Option<tokio::sync::mpsc::Sender<()>>,
    recv_finished: Option<std::sync::mpsc::Receiver<bool>>,
}

lazy_static! {
    static ref RANDOM_ID: Arc<Mutex<String>> = Arc::new(Mutex::new(random_id()));
    static ref STOP_INFORMATION: Arc<Mutex<StopInformation>> = Arc::new(Mutex::new(StopInformation { send_stop_2: None, send_stop_3: None, recv_finished: None }));
    static ref APP_HANDLE: Arc<Mutex<Option<AppHandle>>> = Arc::new(Mutex::new(None));
    static ref LATEST_MY_EVENT: Arc<Mutex<MyEvent>> = Arc::new(Mutex::new(MyEvent { name: "CONNECTING SERVER".to_string() }));
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

#[tauri::command]
fn restart_connection() {
    stop_connection();
    start_connection();
}

#[tauri::command]
fn change_random_id() {
    {
        let mut id = RANDOM_ID.lock().unwrap();
        *id = random_id();
    }
    restart_connection();
}

#[tauri::command]
fn get_latest_my_event() {
    let name;
    {
        name = LATEST_MY_EVENT.lock().unwrap().clone().name;   
    }
    send_event_to_front_end(name.to_string());
}

fn send_event_to_front_end(name: String) {
    let my_event = MyEvent { name };

    {
        let mut e = LATEST_MY_EVENT.lock().unwrap();
        *e = my_event.clone();
    }


    if let Some(app_handle) = APP_HANDLE.lock().unwrap().clone() {
        app_handle.emit_all("my_event", my_event.clone()).unwrap();
    } else {
        println!("Could not emit: {}", my_event.clone().name);
    }
}

fn setup(app: &App) -> Result<(), Box<(dyn std::error::Error + 'static)>> {
    /* {
        let window = app.get_window("main").unwrap();
        window.open_devtools();
        window.close_devtools();
    } */

    //let win = app.get_window("main").unwrap();
    //let _ = win.move_window(Position::BottomRight);
    {
        let mut handle = APP_HANDLE.lock().unwrap();
        *handle = Some(app.handle());
    }

    /* tauri::async_runtime::spawn(async move {
      // listen to the `event-name` (emitted on any window) */
      let id = app.listen_global("event-name", move |event| {
        println!("got event-name with payload {:?}", event.payload());
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

fn start_connection() {
    //let (send_stop_1, recv_stop_1) = channel();
    let (send_stop_2, recv_stop_2) = channel();
    let (send_stop_3, recv_stop_3) = tokio::sync::mpsc::channel::<()>(1);
    //let (send_stop_4, recv_stop_4) = channel();
    let (send_finished, recv_finished) = channel();


    let _main_handle = thread::spawn(move || {
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
                    send_event_to_front_end,
                ).await;
            });
        println!("MAIN PROCESS FINISHED");
    });

    {
        let mut stop_information = STOP_INFORMATION.lock().unwrap();
        stop_information.send_stop_2 = Some(send_stop_2);
        stop_information.send_stop_3 = Some(send_stop_3);
        stop_information.recv_finished = Some(recv_finished);
    }
    
}

fn stop_connection() {
    let stop_information = STOP_INFORMATION.lock().unwrap();

    if let Err(e) = stop_information.send_stop_2.clone().unwrap().send(true) {
        println!("Could not send stop 2 {}", e);
    }
    if let Err(e) = stop_information.send_stop_3.clone().unwrap().try_send(()) {
        println!("Could not send stop 3 {}", e);
    }
    println!("Waiting for main_process to finish");
    let _res = stop_information.recv_finished.as_ref().unwrap().recv(); // result value does not matter here
    
    println!("...Finished");
}

fn main() {
    start_connection();

    /* let open = CustomMenuItem::new("open".to_string(), "Open");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    let tray_menu = SystemTrayMenu::new().add_item(open).add_item(quit);
    let tray = SystemTray::new().with_menu(tray_menu); */

    tauri::Builder::default()
        // .plugin(tauri_plugin_positioner::init())
        /* .system_tray(tray) */
        .setup(|app| setup(app))
        /* .manage(TauriState {
            enigo: Default::default(),
        }) */
        .invoke_handler(tauri::generate_handler![
            get_random_id,
            restart_connection,
            change_random_id,
            get_latest_my_event,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(move |_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { /* api, */ .. } => {
                /* api.prevent_exit(); */
                /* if let Err(e) = send_stop_1.send(true) {
                    println!("Could not send stop 1 {}", e);
                } */
                println!("EXIT REQUESTED");
                stop_connection();
                end_rdev();
            }
            _ => {}
        });
    
    /* println!("----------------- Waiting for main handler to join");
    main_handler.join().expect("Couldn't join main handler"); */
}
