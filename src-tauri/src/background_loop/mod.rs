use serde_json::json;
use tungstenite::stream::MaybeTlsStream;
use std::{net::TcpStream, thread, time, cmp};

use tungstenite::{connect, Message, Error, WebSocket};
use url::Url;

const URL: &str = "ws://localhost:3001"; // "wss://browserkvm-backend.onrender.com"
const SLEEP_ADD_MS: u64 = 500;
const SLEEP_MAX_MS: u64 = 5000;


pub fn start_background_loop () {
    let background_loop_handler = thread::spawn(|| {
        let mut tries: u64 = 0;
        loop {
            let connect_result = connect(Url::parse(URL).unwrap());
            let socket_option: Option<WebSocket<MaybeTlsStream<TcpStream>>> =  match connect_result {
                Ok(result) => {
                    println!("Connected to the server");
                    println!("Response HTTP code: {}", result.1.status());
                    println!("Response contains the following headers:");
                    for (ref header, _value) in result.1.headers() {
                        println!("* {}", header);
                    };
                    Some(result.0)
                },
                Err(error) => {
                    println!("ws connect error: {}", error);
                    None
                },
            };
        
            if let Some(mut socket) = socket_option  {
                tries = 0;

                let msg1 = json!({
                    "operation": "SET_ID",
                    "id": "desktop_1234"
                });
            
                let msg2 = json!({
                    "recipient": "desktop_1234",
                    "content": "content,"
                });
            
            
            
                socket.write_message(Message::Text(msg1.to_string())).unwrap();
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
                }
            } else {
                println!("Could not connect {}...", tries);
                thread::sleep(time::Duration::from_millis(cmp::min(tries * SLEEP_ADD_MS, SLEEP_MAX_MS)));
                tries += 1;
            }
        }
    });
}