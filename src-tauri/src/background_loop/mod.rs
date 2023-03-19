use anyhow::Result;
use serde_json::json;
use tungstenite::stream::MaybeTlsStream;
use webrtc::{api::{media_engine::MediaEngine, interceptor_registry::register_default_interceptors, APIBuilder}, interceptor::registry::Registry, peer_connection::{configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState, math_rand_alpha}, ice_transport::ice_server::RTCIceServer, data::data_channel::{DataChannel, self}, data_channel::{data_channel_message::DataChannelMessage, RTCDataChannel}};
use std::{net::TcpStream, thread, time::{self, Duration}, cmp, sync::Arc};

use tungstenite::{connect, Message, Error, WebSocket};
use url::Url;

const URL: &str = "ws://localhost:3001"; // "wss://browserkvm-backend.onrender.com"
const SLEEP_ADD_MS: u64 = 500;
const SLEEP_MAX_MS: u64 = 5000;

pub fn start_background_loop () {
    let background_loop_handler = thread::spawn(|| {
        let mut tries: u64 = 0;
        loop {
            // Print reconnections, potentially sleep
            println!("Trying to connect {}...", tries);
            thread::sleep(time::Duration::from_millis(cmp::min(tries * SLEEP_ADD_MS, SLEEP_MAX_MS)));
            tries += 1;

            // Connect socket
            let socket_result = connect_socket(URL);
            let socket = match socket_result {
                Ok(socket) => socket,
                Err(e) => {
                    println!("Could not connect socket: {}", e);
                    continue;
                },
            };

            // Connect datachannel and process messages
            let datachannel_option = connect_datachannel(socket);
            /* if let Some(datachannel) = datachannel_option {
                //process_input();
            } else {
                println!("Could not connect datachannel")
            } */
        }
    });
}

fn connect_socket(url: &str) -> Result<WebSocket<MaybeTlsStream<TcpStream>>, Error> {
    let connect_result = connect(Url::parse(URL).unwrap());
    let mut socket = match connect_result {
        Ok(result) => {
            println!("Connected to the server");
            println!("Response HTTP code: {}", result.1.status());
            println!("Response contains the following headers:");
            for (ref header, _value) in result.1.headers() {
                println!("* {}", header);
            };
            result.0
        },
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

fn connect_datachannel(mut socket: WebSocket<MaybeTlsStream<TcpStream>>) -> Result<Arc<RTCDataChannel>, webrtc::Error> {
    // Create a MediaEngine object to configure the supported codec
    let mut m = MediaEngine::default();
    
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

    let result: Result<Arc<RTCDataChannel>, webrtc::Error> = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            // Create a new RTCPeerConnection
            let peer_connection_option = api.new_peer_connection(config).await;
            let peer_connection = match peer_connection_option {
                Ok(peer) => Arc::new(peer),
                Err(e) => return Err(e),
            };

            // Create a datachannel with label 'data'
            let data_channel_option = peer_connection.create_data_channel("data", None).await;
            let data_channel = match data_channel_option {
                Ok(data_channel) => data_channel,
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

            // Register channel opening handling
            let d1 = Arc::clone(&data_channel);
            data_channel.on_open(Box::new(move || {
                println!("Data channel '{}'-'{}' open. Random messages will now be sent to any connected DataChannels every 5 seconds", d1.label(), d1.id());

                let d2 = Arc::clone(&d1);
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
            let d_label = data_channel.label().to_owned();
            data_channel.on_message(Box::new(move |msg: DataChannelMessage| {
                let msg_str = String::from_utf8(msg.data.to_vec()).unwrap();
                println!("Message from DataChannel '{d_label}': '{msg_str}'");
                Box::pin(async {})
            }));

            // Create an offer to send to the browser
            let offer_option = peer_connection.create_offer(None).await;
            let offer = match offer_option {
                Ok(offer) => offer,
                Err(e) => return Err(e),
            };

            // Create channel that is blocked until ICE Gathering is complete
            let mut gather_complete = peer_connection.gathering_complete_promise().await;

            // Sets the LocalDescription, and starts our UDP listeners
            peer_connection.set_local_description(offer).await?;

            // Block until ICE Gathering is complete, disabling trickle ICE
            // we do this because we only can exchange one signaling message
            // in a production application you should exchange ICE Candidates via OnICECandidate
            let _ = gather_complete.recv().await;

            // Output the answer in base64 so we can paste it in browser
            if let Some(local_desc) = peer_connection.local_description().await {
                let json_str = serde_json::to_string(&local_desc).unwrap(); // Not sure if safe
                let b64 = signal::encode(&json_str);
                println!("{b64}");
            } else {
                println!("generate local_description failed!");
            }

            return Ok(data_channel);

            /* let msg2 = json!({
                "recipient": "desktop_1234",
                "content": "content,"
            });

            socket.write_message(Message::Text(msg2.to_string())).unwrap();
            loop {
                let msg = socket.read_message();

                match msg {
                    Ok(message) => println!("ws received: {}", message),
                    Err(error) => {
                        println!("ws error: {}", error);
                        break;
                    },
                };
                //println!("Received: {}", msg);
            } */
        });
    return result;
}