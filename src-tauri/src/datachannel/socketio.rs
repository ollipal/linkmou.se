use std::{rc::Rc, cell::RefCell, sync::{Arc, Mutex}, time::Duration, borrow::Borrow, thread};

use anyhow::Ok;
use rust_socketio::{Payload, Socket, SocketBuilder, RawClient, client::Client };
use serde_json::json;
use tauri::command::private::SerializeKind;
use webrtc::rtp_transceiver::PayloadType;
use std::sync::mpsc;
use std::sync::mpsc::Sender;

fn response_handler(is_done: Sender<bool>, payload: Payload, _socket: RawClient) {
// ... implementation
    // when it's time to wrap up use the channel
    _ = is_done.send(true);
// ...
}

fn make_response_handler(is_done: Sender<bool>) -> impl Fn(Payload, RawClient) {
    move |payload: Payload, socket: RawClient| {
        response_handler(is_done.clone(), payload, socket);
    }
}

fn make_response_handler2(is_done: Sender<String>) -> impl Fn(Payload, RawClient) {
    move |payload: Payload, socket: RawClient| {
        match payload {
            Payload::String(str) => {
                is_done.send(str);
            },
            _ => ()
        }
        //response_handler(is_done.clone(), payload, socket);
    }
}

enum SocketIOState {
    Disconnected,
    Connecting,
    Connected,
}

pub struct SocketIO {
    state: SocketIOState,
    client: Option<Client>,
    on_message: Arc<Mutex<dyn FnMut(String) + Send>>,
}

impl SocketIO {
    pub fn new(on_message: Arc<Mutex<dyn FnMut(String) + Send>>) -> SocketIO {
        SocketIO {
            state: SocketIOState::Disconnected,
            client: Option::None,
            on_message,
        }
    }

    pub fn connect(&mut self, id: &str, on_message2: Arc<Mutex<dyn FnMut(String) + Send>>) {
        println!("connecting");
    
        /* let callback = |payload: Payload, mut socket: Socket| match payload {
            Payload::String(str) => self.on_message.take().unwrap()(str),
            Payload::Binary(bin_data) => println!("bin_data: {:?}", bin_data),
        }; */
        //let on_message_clone= self.on_message.as_mut().unwrap();
    
        let (is_done, rx) = mpsc::channel();
        let handler = make_response_handler2(is_done);


        let socket = SocketBuilder::new("http://localhost:3001")
        //let socket = SocketBuilder::new("https://browserkvm-backend.onrender.com")
            //.on("message", |_payload: Payload, _socket: Socket| on_message_clone("test".to_owned().to_string()))
            .on("message", handler)
            .on("error", |err, _| eprintln!("Error: {:#?}", err))
            .connect()
            .expect("Connection failed");

        socket.emit("setId", id).expect("Server unreachable");;
        
        self.client = Option::Some(socket);

        thread::spawn(move || {
            loop {
                //let mut on_message = self.on_message;
                println!("Waiting receivede");
                let _is_done = rx.recv();
                //println!("Received: {}", _is_done.clone().unwrap());
                on_message2.lock().unwrap()(_is_done.clone().unwrap());
                //self.on_message;
            }
        });
    }
    
    pub fn disconnect(&mut self) {
        // NOTE: this might not work
        println!("disconnecting");
        self.client.as_mut().unwrap().disconnect().expect("Disconnect failed");
    }
    
    pub fn send(&mut self, recipient: &str, content: &str) {
        println!("sending");
        let msg = json!({
            "recipient": recipient,
            "content": content,
        });

        let ack_callback = |message: Payload, _: RawClient| {
            println!("Yehaa! My ack got acked?");
            println!("Ack data: {:#?}", message);
        };

        self.client.as_mut().unwrap().emit_with_ack("message", msg.to_string(), Duration::from_secs(5), ack_callback).expect("Server unreachable");
    }
    

}