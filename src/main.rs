use clap::Parser;
use std::{io::Result, sync::Arc};
use tokio::{
    net::{TcpListener, TcpStream, UnixListener, UnixStream},
    sync::broadcast::{channel, Receiver, Sender},
};
use tracing_subscriber::{util::SubscriberInitExt, FmtSubscriber};

use hermes_gateway::{
    hermes::{self, Feeds, PriceUpdate},
    socket, websocket,
};

#[derive(Parser)]
#[command(about)]
enum Socket {
    /// Run on websocket
    Ws {
        /// Address to listen for ws connections
        #[arg(default_value = "127.0.0.1:7071")]
        address: String,
    },

    /// Run on Unix socket
    Ipc {
        /// Path to listen for ipc connections
        #[arg(default_value = "/tmp/hermes_gateway.ipc")]
        path: String,
    },
}

enum Listener {
    Tcp(TcpListener),
    Unix(UnixListener),
}

enum Stream {
    Tcp(TcpStream),
    Unix(UnixStream),
}

impl Listener {
    async fn bind(socket: Socket) -> Self {
        match socket {
            Socket::Ws { address } => Listener::Tcp(TcpListener::bind(address).await.unwrap()),
            Socket::Ipc { path } => Listener::Unix(UnixListener::bind(path).unwrap()),
        }
    }

    async fn accept(&self) -> Result<Stream> {
        match self {
            Self::Tcp(listener) => listener
                .accept()
                .await
                .map(|(stream, _)| Stream::Tcp(stream)),
            Self::Unix(listener) => listener
                .accept()
                .await
                .map(|(stream, _)| Stream::Unix(stream)),
        }
    }
}

#[tokio::main]
async fn main() {
    let listener: Listener = Listener::bind(Socket::parse()).await;

    FmtSubscriber::builder().compact().finish().init();

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
