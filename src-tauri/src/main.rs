// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use enigo::*;
use rdev::display_size;
use rdev::EventType::MouseMove;
use rdev::{listen, Event};
use serde::Serialize;
use tauri::{App, CustomMenuItem, SystemTray, SystemTrayMenu};
// Use enigo main_display_size when it will be available: https://github.com/enigo-rs/enigo/pull/79
use rust_socketio::{Payload, Socket, SocketBuilder};
use serde_json::json;
use std::io::*;
use std::sync::Mutex;
use std::thread;
use tauri::Manager; // Required to access app in 'setup'
use tauri::State;
use webrtc::ice_transport::ice_credential_type::RTCIceCredentialType;

struct TauriState {
    enigo: Mutex<Enigo>,
}

#[derive(Clone, Serialize)]
struct SystemEvent {
    name: String,
    x: u64,
    y: u64,
    // TODO screen width, height, this logic to front end side
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

#[tokio::main]
async fn main() {
    let _ = create_data_channel().await;

    let open = CustomMenuItem::new("open".to_string(), "Open");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");

    let tray_menu = SystemTrayMenu::new().add_item(open).add_item(quit);
    let tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(tray)
        .setup(|app| setup(app))
        .manage(TauriState {
            enigo: Default::default(),
        })
        .invoke_handler(tauri::generate_handler![init, mouse_move_relative])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                api.prevent_exit();
            }
            _ => {}
        });
}

// Datachannel things below...

use anyhow::Result;
use bytes::Bytes;
use std::sync::Arc;
use tokio::time::Duration;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::math_rand_alpha;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

const MESSAGE_SIZE: usize = 1500;

async fn create_data_channel() -> Result<()> {
    // Everything below is the WebRTC-rs API! Thanks for using it ❤️.

    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();

    // Register default codecs
    m.register_default_codecs()?;

    // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
    // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
    // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
    // for each PeerConnection.
    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut m)?;

    // Since this behavior diverges from the WebRTC API it has to be
    // enabled using a settings engine. Mixing both detached and the
    // OnMessage DataChannel API is not supported.

    // Create a SettingEngine and enable Detach
    let mut s = SettingEngine::default();
    s.detach_data_channels();

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .with_setting_engine(s)
        .build();

    // Prepare the configuration
    let config = RTCConfiguration {
        ice_servers: vec![
            RTCIceServer {
                urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                ..Default::default()
            },
            /* RTCIceServer {
                urls: vec!["turn:TODO".to_owned()],
                username: "TODO".to_owned(),
                credential: "TODO".to_owned(),
                credential_type: RTCIceCredentialType::Password,
            },*/
        ],
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    // Create a datachannel with label 'data'
    let data_channel = peer_connection.create_data_channel("data", None).await?;

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        println!("Peer Connection State has changed: {s}");

        if s == RTCPeerConnectionState::Failed {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            println!("Peer Connection has gone to failed exiting");
            let _ = done_tx.try_send(());
        }

        Box::pin(async {})
    }));

    // Register channel opening handling
    let d = Arc::clone(&data_channel);
    data_channel.on_open(Box::new(move || {
        println!("Data channel '{}'-'{}' open.", d.label(), d.id());

        let d2 = Arc::clone(&d);
        Box::pin(async move {
            let raw = match d2.detach().await {
                Ok(raw) => raw,
                Err(err) => {
                    println!("data channel detach got err: {err}");
                    return;
                }
            };

            // Handle reading from the data channel
            let r = Arc::clone(&raw);
            tokio::spawn(async move {
                let _ = read_loop(r).await;
            });

            // Handle writing to the data channel
            tokio::spawn(async move {
                let _ = write_loop(raw).await;
            });
        })
    }));

    // Create an offer to send to the browser
    let offer = peer_connection.create_offer(None).await?;

    // Create channel that is blocked until ICE Gathering is complete
    let mut gather_complete = peer_connection.gathering_complete_promise().await;

    // Sets the LocalDescription, and starts our UDP listeners
    peer_connection.set_local_description(offer).await?;

    // Block until ICE Gathering is complete, disabling trickle ICE
    // we do this because we only can exchange one signaling message
    // in a production application you should exchange ICE Candidates via OnICECandidate
    let _ = gather_complete.recv().await;

    // Output the offer in base64 so we can paste it in browser
    if let Some(local_desc) = peer_connection.local_description().await {
        let json_str = serde_json::to_string(&local_desc)?;
        let b64 = signal::encode(&json_str);
        println!("{b64}");
        thread::spawn(move || {
            let callback = |payload: Payload, mut socket: Socket| match payload {
                Payload::String(str) => println!("{}", str[1..str.len() - 1].to_string()),
                Payload::Binary(bin_data) => println!("{:?}", bin_data),
            };

            let socket = SocketBuilder::new("http://localhost:3001")
                .on("message", callback)
                .on("error", |err, _| eprintln!("Error: {:#?}", err))
                .connect()
                .expect("Connection failed");

            socket.emit("setId", "test").expect("Server unreachable");
            socket.emit("message", b64).expect("Server unreachable");
        });
    } else {
        println!("generate local_description failed!");
    }

    // Wait for the answer to be pasted
    println!("Waiting for paste...");
    let line = signal::must_read_stdin()?;
    println!("...pasted!");
    let desc_data = signal::decode(line.as_str())?;
    let answer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // Apply the answer as the remote description
    peer_connection.set_remote_description(answer).await?;

    println!("Press ctrl-c to stop");
    tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };

    peer_connection.close().await?;

    Ok(())
}

// read_loop shows how to read from the datachannel directly
async fn read_loop(d: Arc<webrtc::data::data_channel::DataChannel>) -> Result<()> {
    let mut buffer = vec![0u8; MESSAGE_SIZE];
    loop {
        let n = match d.read(&mut buffer).await {
            Ok(n) => n,
            Err(err) => {
                println!("Datachannel closed; Exit the read_loop: {err}");
                return Ok(());
            }
        };

        println!(
            "Message from DataChannel: {}",
            String::from_utf8(buffer[..n].to_vec())?
        );
    }
}

// write_loop shows how to write to the datachannel directly
async fn write_loop(d: Arc<webrtc::data::data_channel::DataChannel>) -> Result<()> {
    let mut result = Result::<usize>::Ok(0);
    while result.is_ok() {
        let timeout = tokio::time::sleep(Duration::from_secs(5));
        tokio::pin!(timeout);

        tokio::select! {
            _ = timeout.as_mut() =>{
                let message = math_rand_alpha(15);
                println!("Sending '{message}'");
                result = d.write(&Bytes::from(message)).await.map_err(Into::into);
            }
        };
    }

    Ok(())
}
