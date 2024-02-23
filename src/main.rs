use std::{
    io::{Error, ErrorKind, Result},
    sync::Arc
};
use tokio::{
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
    sync::broadcast::{channel, Receiver, Sender},
};

use hermes_gateway::{
    hermes::{self, Feeds, PriceUpdate},
    socket,
    websocket
};

enum Listener {
    TcpListener(TcpListener),
    UnixListener(UnixListener)
}

enum Stream {
    TcpStream(TcpStream),
    UnixStream(UnixStream)
}

impl Listener {
    async fn bind(address: String) -> Result<Self> {
        let error: Error;

        match TcpListener::bind(&address).await {
            Ok(l) => return Ok(Listener::TcpListener(l)),
            Err(e) => error = e
        }

        if let ErrorKind::AddrNotAvailable = error.kind() {
            return Err(error)
        }
    
        match UnixListener::bind(address) {
            Ok(l) => Ok(Listener::UnixListener(l)),
            Err(e) => {
                dbg!(error);
                Err(e)
            }
        }
    }

    async fn accept(&self) -> Result<Stream> {
        match self {
            Self::TcpListener(l) => match l.accept().await {
                Ok((stream, _)) => Ok(Stream::TcpStream(stream)),
                Err(e) => Err(e)
            }
            Self::UnixListener(l) => match l.accept().await {
                Ok((stream, _)) => Ok(Stream::UnixStream(stream)),
                Err(e) => Err(e)
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let address: String = match std::env::args().nth(1) {
        Some(a) => a,
        None => "127.0.0.1:7071".to_string()
    };

    let listener: Listener = match Listener::bind(address).await {
        Ok(l) => l,
        Err(e) => {
            dbg!(e);
            return;
        }
    };

    let feeds_store: Arc<Feeds> = Arc::new(Feeds::new());

    let (tx, rx) = channel::<PriceUpdate>(150);

    drop(rx);

    let tx_clone: Sender<PriceUpdate> = tx.clone();

    let feeds_store_clone: Arc<Feeds> = feeds_store.clone();

    tokio::spawn(async move {
        hermes::stream(tx_clone, &feeds_store_clone).await;
    });

    while let Ok(stream) = listener.accept().await {
        let feeds_store_clone: Arc<Feeds> = feeds_store.clone();

        let rx: Receiver<PriceUpdate> = tx.subscribe();

        tokio::spawn(async move {
            match stream {
                Stream::TcpStream(stream) => {
                    websocket::handle_connection(stream, &feeds_store_clone, rx).await
                }
                Stream::UnixStream(stream) => {
                    socket::handle_connection(stream, &feeds_store_clone, rx).await
                }
            }
        });
    }
}
