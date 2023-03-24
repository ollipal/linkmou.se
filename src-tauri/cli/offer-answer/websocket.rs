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

struct WebSocket {
    write: Option<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>,
    read: Option<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    connected: bool,
    url: String,
}

impl WebSocket {
    fn new(url: &str) -> Self {
        WebSocket {
            write: None,
            read: None,
            connected: false,
            url: url.to_string(),
        }
    }

    async fn connect(&mut self) -> Result<(), Error> {
        self.connected = false;

        let (ws_stream, _) = match connect_async(self.url.to_string()).await {
            Ok(socket) => socket,
            Err(e) => return Err(e),
        };

        let (write, read) = ws_stream.split();
        self.write = Some(write);
        self.read = Some(read);

        self.connected = true;
        Ok(())
    }

    async fn recv(&mut self) -> Option<String> {
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
                println!("None message???");
                return None;
            }
        };

        match message.into_text() {
            Ok(text) => Some(text),
            Err(_) => None,
        }
    }

    async fn send(&mut self, message: &str) -> Result<(), Error> {
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

    fn close(&mut self) {
        match &mut self.write {
            Some(write) => { write.close(); },
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
        match websocket.connect().await {
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

        let message = match websocket.recv().await {
            Some(message) => message,
            None => {
                println!("Could not receive");
                continue;
            }
        };

        println!("Received: {}", message);

        websocket.close();
        break;

        //handle.await.expect("The read task failed.");
    }
}
