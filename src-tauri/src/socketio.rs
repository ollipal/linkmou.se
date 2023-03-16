use std::{rc::Rc, cell::RefCell, sync::{Arc, Mutex}, time::Duration, borrow::Borrow};

use rust_socketio::{Payload, Socket, SocketBuilder, RawClient, client::Client};
use serde_json::json;
use webrtc::rtp_transceiver::PayloadType;

enum SocketIOState {
    Disconnected,
    Connecting,
    Connected,
}

pub struct SocketIO {
    state: SocketIOState,
    client: Option<Client>,
    //on_message: Arc<Mutex<dyn Fn(String) -> ()>>,
}

impl SocketIO {
    pub fn new(/* on_message: Arc<Mutex<dyn Fn(String) -> ()>> */) -> SocketIO {
        SocketIO {
            state: SocketIOState::Disconnected,
            client: Option::None,
            //on_message: on_message,
        }
    }

    pub fn connect(&mut self, id: &str) {
        println!("connecting");
    
        let callback = |payload: Payload, mut socket: Socket| match payload {
            Payload::String(str) => println!("Received: {}", str),
            Payload::Binary(bin_data) => println!("bin_data: {:?}", bin_data),
        };
    
        let socket = SocketBuilder::new("http://localhost:3001")
        //let socket = SocketBuilder::new("https://browserkvm-backend.onrender.com")
            .on("message", callback)
            .on("error", |err, _| eprintln!("Error: {:#?}", err))
            .connect()
            .expect("Connection failed");

        socket.emit("setId", id).expect("Server unreachable");;
        self.client = Option::Some(socket);
    }
    
    pub fn disconnect(&mut self, id: &str) {
        println!("disconnecting")
    }
    
    pub fn send(&mut self, recipient: &str, content: &str) {
        println!("sending");
        let msg = json!({
            "recipient": "browser_1234",
            "content": "hello from rust",
        });

        let ack_callback = |message: Payload, _: RawClient| {
            println!("Yehaa! My ack got acked?");
            println!("Ack data: {:#?}", message);
        };

        self.client.as_mut().unwrap().emit_with_ack("message", msg.to_string(), Duration::from_secs(5), ack_callback).expect("Server unreachable");
    }
    

}