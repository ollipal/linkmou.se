

use futures_util::stream::SplitSink;
//use futures_util::stream::SplitStream;
use tokio::net::TcpStream;
use tokio_tungstenite::MaybeTlsStream;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::connect_async;
use futures_util::StreamExt;
use futures_util::SinkExt;
use tungstenite::Error;
//use webrtc::data::message;
use std::sync::mpsc::{Sender, Receiver, channel};
use tokio::task::{JoinHandle};
use tungstenite::Message;

async fn get_socket_write_read_handle(url: &str) -> Result<(SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>, Receiver<String>, JoinHandle<()>), Error> {
  let (receive_sender, receive): (Sender<String>, Receiver<String>) = channel();

  let (ws_stream, _) = match connect_async(url).await {
    Ok(socket) => socket,
    Err(e) => return Err(e),
  };
  
  let (write, mut read) = ws_stream.split();
  
  let handle = tokio::spawn(async move {
    loop {
      let message = match read.next().await {
        Some(result) => match result {
          Ok(message) => message,
          Err(e) => {
            match e {
                Error::ConnectionClosed => (),
                Error::AlreadyClosed => (),
                Error::Protocol(_) => (),
                _ => println!("New error while reading message {}", e),
            }
            break
          },
        },
        None => {
          println!("None message?");
          continue
        },
      };

      println!("We have message");
      if let Err(e) = receive_sender.send(message.to_string()) {
        println!("Error while sending receive {}", e);
        break;
      }
    }
  });

  Ok((write, receive, handle))
}

#[tokio::main]
async fn main() {
  loop {
    let url = "ws://localhost:3001";

    println!("connecting");
    let (mut write, receive, handle) = match get_socket_write_read_handle(url).await {
      Ok(ok) => ok,
      Err(_) => continue,
    };
    println!("connected");
    
    match write.send(tungstenite::Message::Text("test".to_string())).await {
      Ok(_) => (),
      Err(_) => {
        println!("Could not send");
        continue;
      }
    };

    let message = match receive.recv() {
      Ok(message) => message,
      Err(_) => {
        println!("Could not receive");
        continue;
      },
    };

    println!("Received: {}", message);
    
    handle.await.expect("The read task failed.");
  }
}