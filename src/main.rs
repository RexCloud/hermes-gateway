use std::{
    io::{Error, ErrorKind, Result},
    sync::Arc,
};
use tokio::{
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
    sync::broadcast::{channel, Receiver, Sender},
};

use hermes_gateway::{
    hermes::{self, Feeds, PriceUpdate},
    socket, websocket,
};

enum Listener {
    Tcp(TcpListener),
    Unix(UnixListener),
}

enum Stream {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl Listener {
    async fn bind(address: String) -> Result<Self> {
        let error: Error = match TcpListener::bind(&address).await {
            Ok(l) => return Ok(Listener::Tcp(l)),
            Err(e) => e,
        };

        if let ErrorKind::AddrNotAvailable = error.kind() {
            return Err(error);
        }

        match UnixListener::bind(address) {
            Ok(l) => Ok(Listener::Unix(l)),
            Err(e) => {
                dbg!(error);
                Err(e)
            }
        }
    }

    async fn accept(&self) -> Result<Stream> {
        match self {
            Self::Tcp(l) => match l.accept().await {
                Ok((stream, _)) => Ok(Stream::Tcp(stream)),
                Err(e) => Err(e),
            },
            Self::Unix(l) => match l.accept().await {
                Ok((stream, _)) => Ok(Stream::Unix(stream)),
                Err(e) => Err(e),
            },
        }
    }
}

#[tokio::main]
async fn main() {
    let address: String = match std::env::args().nth(1) {
        Some(a) => a,
        None => "127.0.0.1:7071".to_string(),
    };

    let listener: Listener = match Listener::bind(address).await {
        Ok(l) => l,
        Err(e) => {
            dbg!(e);
            return;
        }
    };

    let tx: Sender<PriceUpdate> = channel::<PriceUpdate>(150).0;

    let feeds_store: Arc<Feeds> = Default::default();

    tokio::spawn({
        let tx: Sender<PriceUpdate> = tx.clone();

        let feeds_store: Arc<Feeds> = feeds_store.clone();

        async move {
            hermes::stream(tx, &feeds_store).await;
        }
    });

    while let Ok(stream) = listener.accept().await {
        let feeds_store: Arc<Feeds> = feeds_store.clone();

        let rx: Receiver<PriceUpdate> = tx.subscribe();

        tokio::spawn(async move {
            match stream {
                Stream::Tcp(stream) => websocket::handle_connection(stream, &feeds_store, rx).await,
                Stream::Unix(stream) => socket::handle_connection(stream, &feeds_store, rx).await,
            }
        });
    }
}
