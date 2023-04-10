use anyhow::Result;
use clap::{AppSettings, Arg, Command};
use futures::{FutureExt};
use serde::{Serialize, Deserialize};
use std::io::Write;
use std::sync::mpsc::SyncSender;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use serde_json::json;
use tokio::time::{sleep, Duration};
use std::sync::Arc;
use tokio::sync::Mutex;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::{RTCDataChannel};
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use std::clone::Clone;

mod websocket;
use crate::main_process::datachannel::websocket::{WebSocket, CLOSE, CLOSE_IMMEDIATE};

//const URL: &str = "ws://localhost:3001";
const URL: &str = "wss://browserkvm-backend.onrender.com:443";
const SLEEP_ADD_MS: u64 = 500;
const SLEEP_MAX_MS: u64 = 5000;
//const PING_INTERVAL: u64 = 70;

#[derive(Serialize, Deserialize)]
struct SignalingMessage {
    key: String,
    value: String,
}

lazy_static! {
    static ref PEER_CONNECTION_MUTEX: Arc<Mutex<Option<Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(None));
    static ref PENDING_CANDIDATES: Arc<Mutex<Vec<RTCIceCandidate>>> = Arc::new(Mutex::new(vec![]));
    static ref TX: Arc<Mutex<Option<SyncSender<String>>>> = Arc::new(Mutex::new(None));
}

pub struct MouseOffset {
    pub x: i32,
    pub y: i32,
}

pub struct PostSleepData {
    pub name: String,
    pub mouse_offset: MouseOffset,
}

async fn signal_candidate(c: &RTCIceCandidate) -> Result<()> {
    match c.to_json() {
        Ok(j) => {
            let payload = match serde_json::to_string(&j) {
                Ok(p) => p,
                Err(err) => panic!("{}", err),
            };

            let signaling_message = &json!(SignalingMessage {
                key: "RTCIceCandidate".to_string(),
                value: payload,
            });

            let tx = {
                let tx = TX.lock().await;
                tx.clone()
            };

            match tx {
                Some(tx) => match tx.send(signaling_message.to_string()) {
                    Ok(_) => (),
                    Err(_) => println!("Could not send candidate 1"),
                },
                None => println!("Could not send candidate 2"),
            }    
        },
        Err(_) => println!("Could not send candidate 3"),
    };

    Ok(())
}

//#[tokio::main]
pub async fn process_datachannel_messages<F, G>(enigo_handler_tx: SyncSender<String>, on_message_immmediate: F, on_message_post_sleep: G)
    where
        F: FnOnce(String, SyncSender<String>) -> (Option<u128>, PostSleepData) + std::marker::Sync + std::marker::Send + 'static + std::marker::Copy,
        G: FnOnce(PostSleepData, SyncSender<String>) -> () + std::marker::Sync + std::marker::Send + 'static + std::marker::Copy,
{
    
    //let background_loop_handler = thread::spawn(|| {
    let mut tries: u64 = 0;
    loop {
        // Print reconnections, potentially sleep
        println!("Trying to connect {}...", tries);
        sleep(Duration::from_millis(std::cmp::min(
            tries * SLEEP_ADD_MS,
            SLEEP_MAX_MS,
        )))
        .await;
        tries += 1;

        let mut websocket = WebSocket::new(URL);

        println!("websocket: connecting...");
        if let Err(_) = websocket.connect("desktop_1234".to_string()).await {
            continue;
        };
        tries = 0;
        println!("websocket: ...connected");

        let on_ws_receive = | msg: String | async move {
            println!("websocket: received: {}", msg);

            let signaling_message: SignalingMessage = match serde_json::from_str(&msg) {
                Ok(signaling_message) => signaling_message,
                Err(e) => {
                    println!("Could not serialize websocket message: {}", e);
                    return;
                },
            };

            let pc = {
                let pcm = PEER_CONNECTION_MUTEX.lock().await;
                pcm.clone().unwrap()
            };

            
            if signaling_message.key == "RTCSessionDescription" {
                println!("SDP");
                let sdp_str = &signaling_message.value;

                let sdp = match serde_json::from_str::<RTCSessionDescription>(&sdp_str) {
                    Ok(s) => s,
                    Err(err) => panic!("{}", err),
                };
    
                if let Err(err) = pc.set_remote_description(sdp).await {
                    panic!("{}", err);
                }
    
                // Create an answer to send to the other process
                let answer = match pc.create_answer(None).await {
                    Ok(a) => a,
                    Err(err) => panic!("{}", err),
                };
    
                // Send our answer to the HTTP server listening in the other process
                let payload = match serde_json::to_string(&answer) {
                    Ok(p) => p,
                    Err(err) => panic!("{}", err),
                };
    
    
                let tx = {
                    let tx = TX.lock().await;
                    tx.clone()
                };
            
                let signaling_message = &json!(SignalingMessage {
                    key: "RTCSessionDescription".to_string(),
                    value: payload,
                });
    
                match tx {
                    Some(tx) => match tx.send(signaling_message.to_string()) {
                        Ok(_) => (),
                        Err(_) => println!("Could not send RTCSessionDescription 1"),
                    },
                    None => println!("Could not send RTCSessionDescription 2"),
                }    
                // TODO Return here if any failures

                // Sets the LocalDescription, and starts our UDP listeners
                if let Err(err) = pc.set_local_description(answer).await {
                    panic!("{}", err);
                }
    
                {
                    let cs = PENDING_CANDIDATES.lock().await;
                    for c in &*cs {
                        if let Err(err) = signal_candidate(c).await {
                            panic!("{}", err);
                        }
                    }
                }
            } else if signaling_message.key == "RTCIceCandidate" {
                println!("CANDIDATE");
                let candidate_str = &signaling_message.value;

                let candidate = match serde_json::from_str::<RTCIceCandidateInit>(&candidate_str) {
                    Ok(s) => s,
                    Err(err) => panic!("{}", err),
                };

                if let Err(err) = pc
                    .add_ice_candidate(candidate)
                    .await
                {
                    println!("Could not add_ice_candidate: {}", err);
                }  
            } else {
                println!("Unknown SignalingMessage.key: {}", signaling_message.key);
            }
        }.boxed();

        let (handle, tx) = websocket::start_send_receive_thread(websocket, &"browser_1234".to_string(), on_ws_receive).await;
        {
            let mut tx2 = TX.lock().await;
            *tx2 = Some(tx);
        }

        let result = connect_datachannel_and_process_messages(enigo_handler_tx.clone(), on_message_immmediate, on_message_post_sleep).await.unwrap();

        if let Err(e) = handle.await {
            println!("Handle await error {}", e);
        }

        if result == "CTRLC".to_string() {
            println!("breaking");
            break;
        }
        println!("continue");

        //websocket.close().await;

        
    }
    //});
}

async fn connect_datachannel_and_process_messages<F, G>(enigo_handler_tx: SyncSender<String>, on_message_immmediate: F, on_message_post_sleep: G) -> Result<String>
where
    F: FnOnce(String, SyncSender<String>) -> (Option<u128>, PostSleepData) + std::marker::Sync + std::marker::Send + 'static + std::marker::Copy,
    G: FnOnce(PostSleepData, SyncSender<String>) -> () + std::marker::Sync + std::marker::Send + 'static + std::marker::Copy,
{
    let mut app = Command::new("Answer")
        .version("0.1.0")
        .author("Olli Paloviita")
        .about("browserkwm answer")
        .setting(AppSettings::DeriveDisplayOrder)
        .subcommand_negates_reqs(true)
        .arg(
            Arg::new("FULLHELP")
                .help("Prints more detailed help information")
                .long("fullhelp"),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .help("Prints debug log information"),
        );

    let matches = app.clone().get_matches();

    if matches.is_present("FULLHELP") {
        app.print_long_help().unwrap();
        std::process::exit(0);
    }

    let debug = matches.is_present("debug");
    if debug {
        env_logger::Builder::new()
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{}:{} [{}] {} - {}",
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    record.level(),
                    chrono::Local::now().format("%H:%M:%S.%6f"),
                    record.args()
                )
            })
            .filter(None, log::LevelFilter::Trace)
            .init();
    }

    // Prepare the configuration
    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_owned()],
            ..Default::default()
        }],
        ..Default::default()
    };

    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();
    m.register_default_codecs()?;

    let mut registry = Registry::new();

    // Use the default set of Interceptors
    registry = register_default_interceptors(registry, &mut m)?;

    // Create the API object with the MediaEngine
    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    // When an ICE candidate is available send to the other Pion instance
    // the other Pion instance will add this candidate by calling AddICECandidate
    let pc = Arc::downgrade(&peer_connection);
    let pending_candidates2 = Arc::clone(&PENDING_CANDIDATES);
    peer_connection.on_ice_candidate(Box::new(move |c: Option<RTCIceCandidate>| {
        println!("on_ice_candidate");

        let pc2 = pc.clone();
        let pending_candidates3 = Arc::clone(&pending_candidates2);
        Box::pin(async move {
            if let Some(c) = c {
                if let Some(pc) = pc2.upgrade() {
                    let desc = pc.remote_description().await;
                    if desc.is_none() {
                        let mut cs = pending_candidates3.lock().await;
                        cs.push(c);
                    } else if let Err(err) = signal_candidate(&c).await {
                        panic!("{}", err);
                    }
                }
            }
        })
    }));

    // THIS IS FROM: https://github.com/webrtc-rs/webrtc/blob/master/examples/examples/ice-restart/ice-restart.rs#L112
    // Set the handler for ICE connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_ice_connection_state_change(Box::new(
        |connection_state: RTCIceConnectionState| {
            println!("ICE Connection State has changed: {connection_state}");
            Box::pin(async {})
        },
    ));

    {
        let mut pcm = PEER_CONNECTION_MUTEX.lock().await;
        *pcm = Some(Arc::clone(&peer_connection));
    }

    let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);

    let tx2 = {
        let tx = TX.lock().await;
        tx.clone()
    };

    // Set the handler for Peer connection state
    // This will notify you when the peer has connected/disconnected
    peer_connection.on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
        println!("Peer Connection State has changed: {s}");

        if s == RTCPeerConnectionState::Failed || s == RTCPeerConnectionState::Disconnected {
            // Wait until PeerConnection has had no network activity for 30 seconds or another failure. It may be reconnected using an ICE Restart.
            // Use webrtc.PeerConnectionStateDisconnected if you are interested in detecting faster timeout.
            // Note that the PeerConnection may come back from PeerConnectionStateDisconnected.
            println!("Peer Connection has gone to failed/disconnected exiting");
            let _ = done_tx.try_send(());
        }

        if s == RTCPeerConnectionState::Connected {
            match &tx2 {
                Some(tx) => match tx.send(CLOSE.to_string()) {
                    Ok(_) => (),
                    Err(_) => println!("Could not send CLOSE 1"),
                },
                None => println!("Could not send CLOSE 2"),
            }
        }
        Box::pin(async {})
    }));

    let (done_tx2, mut done_rx2) = tokio::sync::mpsc::channel::<()>(1);

    // Register data channel creation handling
    peer_connection.on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
        let d_label = d.label().to_owned();
        let d_id = d.id();
        println!("New DataChannel {d_label} {d_id}");

        let done_tx2_clone = done_tx2.clone();

        let enigo_handler_tx_clone = enigo_handler_tx.clone();

        /* let ping = json!(Event {
            name: "ping".to_string(),
            value1: None,
            value2: None,
        }).to_string(); */

        Box::pin(async move{
            // Register channel opening handling
            //let d2 =  Arc::clone(&d);
            let d_label2 = d_label.clone();
            let d_id2 = d_id;
            d.on_open(Box::new(move || {
                println!("Data channel '{d_label2}'-'{d_id2}' open. Random messages will now be sent to any connected DataChannels every 5 seconds");
                Box::pin(async /* move */ {
                    /* let mut result = Result::<usize>::Ok(0);
                    while result.is_ok() {
                        // Fix lag spikes: https://stackoverflow.com/a/37144680
                        let timeout = tokio::time::sleep(Duration::from_millis(PING_INTERVAL));
                        tokio::pin!(timeout);

                        tokio::select! {
                            _ = timeout.as_mut() =>{
                                //println!("Sending PING");
                                result = d2.send_text(ping.clone()).await.map_err(Into::into);
                            }
                        };
                    } */
                })
            }));

            // Register text message handling
            d.on_message(Box::new(move |msg: DataChannelMessage| {
                let msg_str = String::from_utf8(msg.data.to_vec()).unwrap();
                //println!("Message from DataChannel '{d_label}': '{msg_str}'");

                let (sleep_amount, post_sleep_data) = on_message_immmediate(msg_str.into(), enigo_handler_tx_clone.clone());
                
                let enigo_handler_tx_clone2 = enigo_handler_tx_clone.clone();
                Box::pin(async move {
                    if let Some(sleep_amount) = sleep_amount {
                        if post_sleep_data.mouse_offset.x == 0 && post_sleep_data.mouse_offset.y == 0 {
                            println!("Zero move sleep skipped")
                        } else {
                            sleep(Duration::from_nanos(sleep_amount.try_into().unwrap())).await;
                        }
                    }
                    on_message_post_sleep(post_sleep_data, enigo_handler_tx_clone2);
                })
            }));

            d.on_close(Box::new(move || {
                println!("DC CLOSE");
                let _ = done_tx2_clone.try_send(());
                Box::pin(async{})
            }));
        })
    }));

    println!("Press ctrl-c to stop");
    let result = tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
            "DISCONNECT"
        }
        _ = done_rx2.recv() => {
            println!("received done signal!");
            "DISCONNECT"
        }
        _ = tokio::signal::ctrl_c() => {
            println!("CTRLC");
            "CTRLC"
        }
    };

    let tx = {
        let tx = TX.lock().await;
        tx.clone()
    };
    
    match tx {
        Some(tx) => match tx.send(CLOSE_IMMEDIATE.to_string()) {
            Ok(_) => (),
            Err(_) => println!("Could not send CLOSE"),
        },
        None => println!("Could not send CLOSE"),
    }

    println!("closing peer");
    peer_connection.close().await?;
    println!("closed peer");

    Ok(result.to_string())
}
