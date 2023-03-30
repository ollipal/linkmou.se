use anyhow::Result;
use clap::{AppSettings, Arg, Command};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Client, Method, Request, Response, Server, StatusCode};
use serde_json::json;
/* use tungstenite::{WebSocket, connect, Message};
use tungstenite::stream::MaybeTlsStream; */
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::{cmp};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::ice_transport::ice_candidate::{RTCIceCandidate, RTCIceCandidateInit};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::interceptor::registry::Registry;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::math_rand_alpha;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;

use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

mod websocket;
use crate::websocket::WebSocket;

const URL: &str = "ws://localhost:3001"; // "wss://browserkvm-backend.onrender.com"
const SLEEP_ADD_MS: u64 = 500;
const SLEEP_MAX_MS: u64 = 5000;
const WEBSOCKET_MESSAGE_CHECK_DELAY: u64 = 1000;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref PEER_CONNECTION_MUTEX: Arc<Mutex<Option<Arc<RTCPeerConnection>>>> =
        Arc::new(Mutex::new(None));
    static ref PENDING_CANDIDATES: Arc<Mutex<Vec<RTCIceCandidate>>> = Arc::new(Mutex::new(vec![]));
    static ref ADDRESS: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    static ref TX: Arc<Mutex<Option<SyncSender<String>>>> = Arc::new(Mutex::new(None));
}

async fn signal_candidate(addr: &str, c: &RTCIceCandidate) -> Result<()> {
    println!(
        "signal_candidate Post candidate to {}",
        format!("http://{}/candidate", addr)
    );
    let payload = c.to_json()?.candidate;
    let req = match Request::builder()
        .method(Method::POST)
        .uri(format!("http://{addr}/candidate"))
        .header("content-type", "text/plain; charset=utf-8")
        //.body(Body::from(signal::encode(&payload.to_string())))
        .body(Body::from(payload.to_string()))
    {
        Ok(req) => req,
        Err(err) => {
            println!("{err}");
            return Err(err.into());
        }
    };

    let _resp = match Client::new().request(req).await {
        Ok(resp) => resp,
        Err(err) => {
            println!("{err}");
            return Err(err.into());
        }
    };
    //println!("signal_candidate Response: {}", resp.status());

    let tx = {
        let tx = TX.lock().await;
        tx.clone()
    };

    match c.to_json() {
        Ok(j) => {
            match tx {
                Some(tx) => match tx.send(j.candidate.to_string()) {
                    Ok(_) => (),
                    Err(_) => todo!(),
                },
                None => println!("Could not send candidate"),
            }    
        },
        Err(_) => todo!(),
    };

    Ok(())
}

// HTTP Listener to get ICE Credentials/Candidate from remote Peer
async fn remote_handler(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let pc = {
        let pcm = PEER_CONNECTION_MUTEX.lock().await;
        pcm.clone().unwrap()
    };
    let addr = {
        let addr = ADDRESS.lock().await;
        addr.clone()
    };

    match (req.method(), req.uri().path()) {
        // A HTTP handler that allows the other WebRTC-rs or Pion instance to send us ICE candidates
        // This allows us to add ICE candidates faster, we don't have to wait for STUN or TURN
        // candidates which may be slower
        (&Method::POST, "/candidate") => {
            //println!("remote_handler receive from /candidate");
            let candidate =
                match std::str::from_utf8(&hyper::body::to_bytes(req.into_body()).await?) {
                    Ok(s) => s.to_owned(),
                    Err(err) => panic!("{}", err),
                };

            if let Err(err) = pc
                .add_ice_candidate(RTCIceCandidateInit {
                    candidate,
                    ..Default::default()
                })
                .await
            {
                panic!("{}", err);
            }

            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::OK;
            Ok(response)
        }

        // A HTTP handler that processes a SessionDescription given to us from the other WebRTC-rs or Pion process
        (&Method::POST, "/sdp") => {
            //println!("remote_handler receive from /sdp");
            let sdp_str = match std::str::from_utf8(&hyper::body::to_bytes(req.into_body()).await?)
            {
                Ok(s) => s.to_owned(),
                Err(err) => panic!("{}", err),
            };
            let sdp = match serde_json::from_str::<RTCSessionDescription>(&sdp_str) {
                Ok(s) => s,
                Err(err) => panic!("{}", err),
            };

            if let Err(err) = pc.set_remote_description(sdp).await {
                panic!("{}", err);
            }

            {
                let cs = PENDING_CANDIDATES.lock().await;
                for c in &*cs {

                    if let Err(err) = signal_candidate(&addr, c).await {
                        panic!("{}", err);
                    }
                }
            }

            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::OK;
            Ok(response)
        }
        // Return the 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

async fn read_message(websocket: &mut WebSocket) -> Option<String> {
    let msg = websocket.recv().await;

    if msg.is_none() {
        wait().await;
        wait().await;
    }
    msg
}

async fn wait() {
    sleep(Duration::from_millis(WEBSOCKET_MESSAGE_CHECK_DELAY)).await;
}

#[tokio::main]
async fn main() {
    //let background_loop_handler = thread::spawn(|| {
    let mut tries: u64 = 0;
    loop {
        // Print reconnections, potentially sleep
        println!("Trying to connect {}...", tries);
        sleep(Duration::from_millis(cmp::min(
            tries * SLEEP_ADD_MS,
            SLEEP_MAX_MS,
        )))
        .await;
        tries += 1;

        let url = "ws://localhost:3001";
        let mut websocket = WebSocket::new(url);

        println!("connecting");
        match websocket.connect("browser_1234".to_string()).await {
            Ok(ok) => ok,
            Err(_) => continue,
        };
        println!("connected");

        match websocket.send("test").await {
            Ok(_) => (),
            Err(_) => {
                println!("Could not send");
                continue;
            }
        };

        /* let message = match websocket.recv().await {
            Some(message) => message,
            None => {
                println!("Could not receive");
                continue;
            }
        };

        println!("Received: {}", message); */

        //websocket.close().await;

        let (tx, rx) : (SyncSender<String>, Receiver<String>) = sync_channel(1);

        {
            let mut tx2 = TX.lock().await;
            *tx2 = Some(tx);
        }

        //let shared_tx = Arc::new(tx);


        let handle = tokio::spawn(async move {
            println!("SPAWNED");
            loop {
                let msg = rx.try_iter().next();
    
                if msg.is_some() {
                    let msg = msg.unwrap();
                    println!("SENDING: {}", msg);
                    websocket.send(&msg).await.unwrap();
                    // TODO stop loop if connected
                }
                
                // TODO handle close (call websocket close)
                // TODO handle connected (sleep 1000 ms?)

                tokio::select! {
                    msg = read_message(&mut websocket) => {
                        match msg {
                            Some(msg) => println!("msg received: {}", msg),
                            None => println!("None received"),
                        }
                        
                    }
                    _ = wait() => {
                        println!("timeout")
                    }
                };

                

                
                //println!("100 ms have elapsed");
            }
            
        });


        //let shared_rx = Arc::new(rx);

        old_main(/* shared_tx */).await.unwrap();

        //websocket.close().await;

        // TODO send stop
        // TODO join handle

        break;
    }
    //});
}

async fn old_main(/* shared_tx: Arc<SyncSender<String>> */) -> Result<()> {
    let mut app = Command::new("Offer")
        .version("0.1.0")
        .author("Rain Liu <yliu@webrtc.rs>")
        .about("An example of WebRTC-rs Offer.")
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
        )
        .arg(
            Arg::new("offer-address")
                .takes_value(true)
                .default_value("0.0.0.0:50000")
                .long("offer-address")
                .help("Address that the Offer HTTP server is hosted on."),
        )
        .arg(
            Arg::new("answer-address")
                .takes_value(true)
                .default_value("localhost:60000")
                .long("answer-address")
                .help("Address that the Answer HTTP server is hosted on."),
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

    let offer_addr = matches.value_of("offer-address").unwrap().to_owned();
    let answer_addr = matches.value_of("answer-address").unwrap().to_owned();

    {
        let mut oa = ADDRESS.lock().await;
        *oa = answer_addr.clone();
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
    let addr2 = answer_addr.clone();

    /* let tx = Arc::clone(&shared_tx); */

    peer_connection.on_ice_candidate(Box::new(move |c: Option<RTCIceCandidate>| {
        /* println!("on_ice_candidate"); */
        
        
        /* let tx = Arc::clone(&tx); */
        /* tx.send("on_ice_candidate".to_string()).unwrap(); */

        let pc2 = pc.clone();
        let pending_candidates3 = Arc::clone(&pending_candidates2);
        let addr3 = addr2.clone();
        Box::pin(async move {
            if let Some(c) = c {
                if let Some(pc) = pc2.upgrade() {

                    println!("ACTUAL -------");

                    let desc = pc.remote_description().await;
                    if desc.is_none() {
                        let mut cs = pending_candidates3.lock().await;
                        cs.push(c);

                        println!("ice candidate");

                    } else if let Err(err) = signal_candidate(&addr3, &c).await {
                        panic!("{}", err);
                    }
                }
            }
        })
    }));

    println!("Listening on http://{offer_addr}");
    {
        let mut pcm = PEER_CONNECTION_MUTEX.lock().await;
        *pcm = Some(Arc::clone(&peer_connection));
    }

    tokio::spawn(async move {
        let addr = SocketAddr::from_str(&offer_addr).unwrap();
        let service =
            make_service_fn(|_| async { Ok::<_, hyper::Error>(service_fn(remote_handler)) });
        let server = Server::bind(&addr).serve(service);
        // Run this server for... forever!
        if let Err(e) = server.await {
            eprintln!("server error: {e}");
        }
    });


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

    // Create an offer to send to the other process
    let offer = peer_connection.create_offer(None).await?;

    // Send our offer to the HTTP server listening in the other process
    let payload = match serde_json::to_string(&offer) {
        Ok(p) => p,
        Err(err) => panic!("{}", err),
    };

    // Sets the LocalDescription, and starts our UDP listeners
    // Note: this will start the gathering of ICE candidates
    peer_connection.set_local_description(offer).await?;

    //println!("Post: {}", format!("http://{}/sdp", answer_addr));
    let req = match Request::builder()
        .method(Method::POST)
        .uri(format!("http://{answer_addr}/sdp"))
        .header("content-type", "text/plain; charset=utf-8")
        //.body(Body::from(signal::encode(&payload.to_string())))
        .body(Body::from(payload.to_string()))
    {
        Ok(req) => req,
        Err(err) => panic!("{}", err),
    };

    let _resp = match Client::new().request(req).await {
        Ok(resp) => resp,
        Err(err) => {
            println!("{err}");
            return Err(err.into());
        }
    };
    println!("Response: {}", _resp.status());

    let tx = {
        let tx = TX.lock().await;
        tx.clone()
    };

    match tx {
        Some(tx) => match tx.send(payload.to_string()) {
            Ok(_) => (),
            Err(_) => todo!(),
        },
        None => println!("Could not send sdp"),
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

    peer_connection.close().await?;

    Ok(())
}
