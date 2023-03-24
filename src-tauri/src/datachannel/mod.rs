// Datachannel things below...

use anyhow::Result;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::thread;
use std::{io::*, time};
use tokio::time::Duration;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_credential_type::RTCIceCredentialType;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::{math_rand_alpha, RTCPeerConnection};

use std::sync::mpsc::Sender;
use std::sync::mpsc::{self, Receiver};

mod socketio;
use crate::datachannel::socketio::SocketIO;

const MESSAGE_SIZE: usize = 1500;

#[derive(Serialize, Deserialize)]
struct SocketIOMessage {
    key: String,
    value: String,
}

pub async fn create_data_channel() -> Result<()> {
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
    let peer_connection: Arc<RTCPeerConnection> = Arc::new(api.new_peer_connection(config).await?);

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

    peer_connection.on_data_channel(Box::new(
        move |data_channel: Arc<webrtc::data_channel::RTCDataChannel>| {
            println!(
                "Data channel '{}'-'{}' open.",
                data_channel.label(),
                data_channel.id()
            );

            Box::pin(async move {
                let raw = match data_channel.detach().await {
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
        },
    ));
    /* let d2 = Arc::clone(&d);
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
    }) */
    //}));

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

        // Create a new runtime for blocking calls
        let rt = tokio::runtime::Runtime::new().unwrap();

        thread::spawn(move || {
            //let pc_clone = Arc::clone(&peer_connection);

            let on_message = move |message: String| {
                //let v: SocketIOMessage = serde_json::from_str(&mes§sage).unwrap();

                // remove " from start and end
                let mut chars = message.chars();
                chars.next();
                chars.next_back();

                let mut splitted = chars.as_str().split('|');
                let key = splitted.next().unwrap();
                let value = splitted.next().unwrap().to_string();
                if key == "RTCSessionDescription" {
                    println!("Message received 0: {}", value);

                    let desc_data = signal::decode(value.as_str()).unwrap();

                    println!("Message received 1: {}", desc_data);

                    let answer = serde_json::from_str::<RTCSessionDescription>(&desc_data).unwrap();

                    println!("Message received 2: {:?}", answer);

                    // Apply the answer as the remote description
                    println!("BEFORE");
                    let result = rt.block_on(async {
                        peer_connection
                            .set_remote_description(answer)
                            .await
                            .unwrap();

                        /* tokio::select! {
                            _ = done_rx.recv() => {
                                println!("received done signal!");
                            }
                            _ = tokio::signal::ctrl_c() => {
                                println!();
                            }
                        }; */
                    });
                    println!("AFTER");

                    /* let desc_data = signal::decode(key).unwrap();
                    println!("{}", desc_data); */

                    /* set_session_description(peer_connection, value.clone().to_string()); */
                } else {
                    println!("Other Message received: {}", key);
                }

                //set_session_description(peer_connection, message);
                /* if false {
                    set_session_description(peer_connection);
                } else {
                    //println!("else");
                } */
            };

            let mut s = SocketIO::new(/* Arc::new(Mutex::new(on_message)) */);
            s.connect("desktop_1234", Arc::new(Mutex::new(on_message)));
            thread::sleep(time::Duration::from_millis(1000));

            //loop {
            s.send(
                "browser_1234",
                &json!(SocketIOMessage {
                    key: "RTCSessionDescription".to_string(),
                    value: b64.to_string()
                })
                .to_string(),
            );
            thread::sleep(time::Duration::from_millis(5000));
            s.send(
                "browser_1234",
                &json!(SocketIOMessage {
                    key: "RTCSessionDescription".to_string(),
                    value: b64.to_string()
                })
                .to_string(),
            );
            thread::sleep(time::Duration::from_millis(5000));
            //s.disconnect();
            //}
        });
    } else {
        println!("generate local_description failed!");
    }
    Ok(())
}

/* async fn set_session_description(peer_connection: Arc<RTCPeerConnection>, desc_data: String/* , mut done_rx: Receiver<()> */) -> Result<()> {
    // Wait for the answer to be pasted
    //println!("Waiting for paste...");
    //let line = signal::must_read_stdin()?;
    //println!("...pasted!");
    //let desc_data = signal::decode(line.as_str())?;
    let answer = serde_json::from_str::<RTCSessionDescription>(&desc_data)?;

    // Apply the answer as the remote description
    peer_connection.set_remote_description(answer).await?;

    println!("Press ctrl-c to stop");
    /* tokio::select! {
        _ = done_rx.recv() => {
            println!("received done signal!");
        }
        _ = tokio::signal::ctrl_c() => {
            println!();
        }
    };

    peer_connection.close().await?; */
    Ok(())
} */

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
