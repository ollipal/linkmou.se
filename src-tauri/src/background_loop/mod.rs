use anyhow::Result;
use serde_json::json;
use std::{
    cmp,
    net::TcpStream,
    sync::Arc,
    thread,
    time::{self, Duration},
};
use tungstenite::stream::MaybeTlsStream;
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors, media_engine::MediaEngine, APIBuilder,
    },
    data::data_channel::{self, DataChannel},
    data_channel::{data_channel_message::DataChannelMessage, RTCDataChannel},
    ice_transport::{ice_candidate::RTCIceCandidate, ice_server::RTCIceServer},
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration, math_rand_alpha,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
    },
};

use tungstenite::{connect, Message, WebSocket};
use url::Url;

const URL: &str = "ws://localhost:3001"; // "wss://browserkvm-backend.onrender.com"
const SLEEP_ADD_MS: u64 = 500;
const SLEEP_MAX_MS: u64 = 5000;

pub fn start_background_loop() {
    let background_loop_handler = thread::spawn(|| {
        let mut tries: u64 = 0;
        loop {
            // Print reconnections, potentially sleep
            println!("Trying to connect {}...", tries);
            thread::sleep(time::Duration::from_millis(cmp::min(
                tries * SLEEP_ADD_MS,
                SLEEP_MAX_MS,
            )));
            tries += 1;

            // Connect socket
            let socket_result = connect_socket(URL);
            let socket = match socket_result {
                Ok(socket) => socket,
                Err(e) => {
                    println!("Could not connect socket: {}", e);
                    continue;
                }
            };

            // Read socket
            //let rtc_session_description : RTCSessionDescription = read_rtc_session_description(socket);

            // Connect datachannel and process messages
            let rtc_local_description =
                connect_datachannel_and_start_background_processing(rtc_session_description);

            // Write socket
            //let send_success = send_rtc_local_description(socket, rtc_local_description);

            /* if let Some(datachannel) = datachannel_option {
                //process_input();
            } else {
                println!("Could not connect datachannel")
            } */
        }
    });
}

fn connect_socket(url: &str) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, tungstenite::Error> {
    let connect_result = connect(Url::parse(URL).unwrap());
    let mut socket = match connect_result {
        Ok(result) => {
            println!("Connected to the server");
            println!("Response HTTP code: {}", result.1.status());
            println!("Response contains the following headers:");
            for (ref header, _value) in result.1.headers() {
                println!("* {}", header);
            }
            result.0
        }
        Err(e) => return Err(e.into()),
    };

    // Set socket id
    // This enables receiving messages
    let set_id_message = json!({
        "operation": "SET_ID",
        "id": "desktop_1234"
    });

    match socket.write_message(Message::Text(set_id_message.to_string())) {
        Ok(_) => Ok(socket),
        Err(e) => Err(e.into()),
    }
}

fn connect_datachannel_and_start_background_processing(
    offer: RTCSessionDescription,
) -> Result<RTCSessionDescription, webrtc::Error> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // Create a MediaEngine object to configure the supported codec
            let mut m = MediaEngine::default();
            // Register default codecs
            match m.register_default_codecs() {
                Ok(_) => (),
                Err(e) => return Err(e),
            };

            // Create a InterceptorRegistry. This is the user configurable RTP/RTCP Pipeline.
            // This provides NACKs, RTCP Reports and other features. If you use `webrtc.NewPeerConnection`
            // this is enabled by default. If you are manually managing You MUST create a InterceptorRegistry
            // for each PeerConnection.
            let mut registry = Registry::new();
            // Use the default set of Interceptors
            registry = match register_default_interceptors(registry, &mut m) {
                Ok(registry) => registry,
                Err(e) => return Err(e),
            };

            // Create the API object with the MediaEngine
            let api = APIBuilder::new()
                .with_media_engine(m)
                .with_interceptor_registry(registry)
                .build();

            // Prepare the configuration
            let config = RTCConfiguration {
                ice_servers: vec![RTCIceServer {
                    urls: vec!["stun:stun.l.google.com:19302".to_owned()],
                    ..Default::default()
                }],
                ..Default::default()
            };

            // Create a new RTCPeerConnection
            let peer_connection_option = api.new_peer_connection(config).await;
            let peer_connection = match peer_connection_option {
                Ok(peer_connection) => Arc::new(peer_connection),
                Err(e) => return Err(e),
            };

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

            // Register data channel creation handling
            peer_connection
                .on_data_channel(Box::new(move |d: Arc<RTCDataChannel>| {
                    let d_label = d.label().to_owned();
                    let d_id = d.id();
                    println!("New DataChannel {d_label} {d_id}");

                    // Register channel opening handling
                    Box::pin(async move {
                        let d2 = Arc::clone(&d);
                        let d_label2 = d_label.clone();
                        let d_id2 = d_id;
                        d.on_open(Box::new(move || {
                            println!("Data channel '{d_label2}'-'{d_id2}' open. Random messages will now be sent to any connected DataChannels every 5 seconds");

                            Box::pin(async move {
                                let mut result = Result::<usize>::Ok(0);
                                while result.is_ok() {
                                    let timeout = tokio::time::sleep(Duration::from_secs(5));
                                    tokio::pin!(timeout);

                                    tokio::select! {
                                        _ = timeout.as_mut() =>{
                                            let message = math_rand_alpha(15);
                                            println!("Sending '{message}'");
                                            result = d2.send_text(message).await.map_err(Into::into);
                                        }
                                    };
                                }
                            })
                        }));

                        // Register text message handling
                        d.on_message(Box::new(move |msg: DataChannelMessage| {
                            let msg_str = String::from_utf8(msg.data.to_vec()).unwrap();
                            println!("Message from DataChannel '{d_label}': '{msg_str}'");
                            Box::pin(async {})
                        }));
                    })
                }));

                // Wait for the offer to be pasted
                //let line = signal::must_read_stdin()?;
                //let desc_data = signal::decode(line.as_str())?;
                //let offer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

                // Set the remote SessionDescription
                peer_connection.set_remote_description(offer).await?;

                // Create an answer
                let answer = peer_connection.create_answer(None).await?;

                // Create channel that is blocked until ICE Gathering is complete
                let mut gather_complete = peer_connection.gathering_complete_promise().await;

                // Sets the LocalDescription, and starts our UDP listeners
                peer_connection.set_local_description(answer).await?;


                peer_connection.on_ice_candidate(Box::new(move |c: Option<RTCIceCandidate>| {
                    println!("on_ice_candidate {:?} TODO USE THIS FOR SOMETHING!", c);
                    Box::pin(async move {
                        // https://github.com/webrtc-rs/webrtc/blob/master/examples/examples/offer-answer/answer.rs#L283
                    })
                }));
            

                // Block until ICE Gathering is complete, disabling trickle ICE
                // we do this because we only can exchange one signaling message
                // in a production application you should exchange ICE Candidates via OnICECandidate
                match gather_complete.recv().await {
                    None => return Err(webrtc::Error::new("Gathering failed".to_string())),
                    _ => (),
                }


                // Output the answer in base64 so we can paste it in browser
                /* if let Some(local_desc) = peer_connection.local_description().await {
                    let json_str = serde_json::to_string(&local_desc).unwrap(); // Is this safe?
                    let b64 = signal::encode(&json_str);
                    println!("{b64}");
                } else {
                    println!("generate local_description failed!");
                }

                println!("Press ctrl-c to stop");
                tokio::select! {
                    _ = done_rx.recv() => {
                        println!("received done signal!");
                    }
                    _ = tokio::signal::ctrl_c() => {
                        println!();
                    }
                };

                peer_connection.close().await?; */

                match peer_connection.local_description().await {
                    Some(local_description) => Ok(local_description),
                    None => Err(webrtc::Error::new("Could not get local_description".to_string())),
                }
            }
        )
}
