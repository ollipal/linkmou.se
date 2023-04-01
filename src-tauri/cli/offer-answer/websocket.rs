use futures::{future::BoxFuture, FutureExt};
use serde_json::json;
use std::sync::Arc;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use tokio::task::JoinHandle;

use futures_util::stream::SplitSink;
use futures_util::stream::SplitStream;
use futures_util::SinkExt;
use futures_util::StreamExt;
use tokio::net::TcpStream;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tungstenite::Error;
use tungstenite::Message;
use tokio::time::{sleep, Duration};

pub struct WebSocket {
    write: Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    read: Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    url: String,
}

const WEBSOCKET_MESSAGE_CHECK_DELAY: u64 = 1000;

async fn read_message(websocket: &mut WebSocket) -> Option<String> {
    let msg = websocket.recv().await;

    if msg.is_none() { // Give more time for wait() to finish...
        wait(2 * WEBSOCKET_MESSAGE_CHECK_DELAY).await;
    }

    msg
}

async fn wait(duration: u64) {
    sleep(Duration::from_millis(duration)).await;
}

pub async fn start_send_receive_thread<C>(mut websocket: WebSocket, on_ws_receive: C) -> (tokio::task::JoinHandle<()>, SyncSender<std::string::String>)
where
    C: FnOnce(String) -> BoxFuture<'static, ()> + 'static + std::marker::Copy + std::marker::Send,
    // BoxFuture tip from here: https://www.bitfalter.com/async-closures
{

    let (send_websocket, rx) : (SyncSender<String>, Receiver<String>) = sync_channel(1);

    let thread_handle = tokio::spawn(async move {
        println!("websocket thread spawn");
        loop {
            let msg = rx.try_iter().next();

            if msg.is_some() {
                let msg = msg.unwrap();
                println!("websocket: sending: {}", msg);

                if let Err(err) = websocket.send(&msg).await {
                    println!("websocket: could not send, {}", err)
                    // TODO end thread?
                };
            }
            
            // TODO thread_handle close (call websocket close)
            // TODO thread_handle connected (sleep 1000 ms?)

            tokio::select! {
                msg = read_message(&mut websocket) => {
                    match msg {
                        None => println!("websocket: received None"),
                        Some(msg) => {
                            println!("websocket: received: {}", msg);


                            //on_ws_receive(msg).boxed_local(); 
                            on_ws_receive(msg).await;
                            //copy.lock().await;
                        },
                    }
                    
                }
                // TODO change the duration to longer if connected for a while
                // or even cut the connection
                _ = wait(WEBSOCKET_MESSAGE_CHECK_DELAY) => {
                    println!("timeout")
                }
            };
            //println!("100 ms have elapsed");
        }
        println!("websocket thread end");
    });

    return (thread_handle, send_websocket);
} 

impl WebSocket {
    pub fn new(url: &str) -> Self {
        WebSocket {
            write: None,
            read: None,
            url: url.to_string(),
        }
    }

    pub async fn connect(&mut self, id: String) -> Result<(), Error> {
        let (ws_stream, _) = match connect_async(self.url.to_string()).await {
            Ok(socket) => socket,
            Err(e) => return Err(e),
        };

        let (mut write, read) = ws_stream.split();
        
        let set_id_message = json!({
            "operation": "SET_ID",
            "id": id
        });
    
        if let Err(e) = write.send(Message::Text(set_id_message.to_string())).await {
            return Err(e);
        };
        
        self.write = Some(write);
        self.read = Some(read);

        Ok(())
    }

    pub async fn recv(&mut self) -> Option<String> {
        let read = match &mut self.read {
            Some(read) => read,
            None => return None,
        };

        let message = match read.next().await {
            Some(result) => match result {
                Ok(message) => message,
                Err(e) => {
                    match e {
                        Error::ConnectionClosed => (),
                        Error::AlreadyClosed => (),
                        Error::Protocol(_) => (),
                        _ => println!("New error while reading message {}", e),
                    };
                    return None;
                }
            },
            None => {
                println!("Websocket has disconnected most likely");
                return None;
            }
        };

        match message.into_text() {
            Ok(text) => Some(text),
            Err(_) => None,
        }
    }

    pub async fn send(&mut self, message: &str) -> Result<(), Error> {
        let write = match &mut self.write {
            Some(write) => write,
            None => return Err(Error::AlreadyClosed),
        };

        match write
            .send(tungstenite::Message::Text(message.to_string()))
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub async fn close(&mut self) {
        match &mut self.write {
            Some(write) => {
                match write.send(tungstenite::Message::Text("CLOSE".to_string())).await
                {
                    Ok(_) => (),
                    Err(_) => (),
                };
                match write.close().await {
                    Ok(_) => (),
                    Err(_) => (),
                };
            },
            None => (),
        }
        // TODO how to close the read side?
    }
}

#[tokio::main]
async fn main() {
    loop {
        let url = "ws://localhost:3001";
        let mut websocket = WebSocket::new(url);

        println!("connecting");
        if let Err(_) = websocket.connect("desktop_1234".to_string()).await {
            continue;
        };
        println!("connected");

        if let Err(_) = websocket.send("test").await {
            println!("Could not send");
            continue;
        };

        let message = match websocket.recv().await {
            Some(message) => message,
            None => {
                println!("Could not receive");
                continue;
            }
        };

        println!("Received: {}", message);

        websocket.close().await;
        break;

        //thread_handle.await.expect("The read task failed.");
    }
}
