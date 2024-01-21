use futures_util::{
    FutureExt,
    SinkExt,
    StreamExt,
    TryStreamExt
};
use primitive_types::H256;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    collections::HashSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        Mutex
    }
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast::{channel, Sender, Receiver},
    time::{sleep, Duration}
};
use tokio_tungstenite::{
    accept_async,
    connect_async,
    tungstenite::Message
};

#[derive(Debug, Deserialize, Serialize)]
struct Subscription {
    ids: Vec<H256>,
    r#type: String,
    verbose: bool,
    binary: bool
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Price {
    conf: String,
    expo: i32,
    price: String,
    publish_time: u32
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PriceFeed {
    ema_price: Price,
    id: H256,
    price: Price,
    vaa: String
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PriceUpdate {
    r#type: String,
    price_feed: PriceFeed
}

struct Feeds {
    ids: Mutex<Vec<Vec<H256>>>,
    modified: AtomicBool
}

impl Feeds {
    fn new() -> Self {
        Self {
            ids: Mutex::new(Vec::new()),
            modified: AtomicBool::new(false)
        }
    }

    fn add(&self, ids: Vec<H256>) {
        self.ids.lock().unwrap().push(ids);
        
        self.modified.store(true, Ordering::SeqCst);
    }

    fn remove(&self, ids: &Vec<H256>) {
        let mut feeds_lock = self.ids.lock().unwrap();

        match feeds_lock.iter().position(|feeds| *feeds == *ids) {
            Some(index) => {
                feeds_lock.remove(index);
                self.modified.store(true, Ordering::SeqCst);
            },
            None => {
                dbg!(&feeds_lock);
                dbg!(ids);
            }
        }
    }
}

const HERMES_WSS_URL: &str = "wss://hermes.pyth.network/ws";

#[tokio::main]
async fn main() {
    let address: String = match std::env::args().nth(1) {
        Some(a) => a,
        None => "127.0.0.1:7071".to_string()
    };

    let listener: TcpListener = TcpListener::bind(address)
        .await
        .expect("can't bind listener to provided address");

    let feeds_store: Arc<Feeds> = Arc::new(Feeds::new());

    let (tx, rx) = channel::<PriceUpdate>(150);

    drop(rx);

    let tx_clone: Sender<PriceUpdate> = tx.clone();

    let feeds_store_clone: Arc<Feeds> = feeds_store.clone();

    tokio::spawn(async move {
        hermes_stream(tx_clone, &feeds_store_clone).await;
    });

    while let Ok((stream, _)) = listener.accept().await {
        let feeds_store_clone: Arc<Feeds> = feeds_store.clone();

        let rx: Receiver<PriceUpdate> = tx.subscribe();

        tokio::spawn(async move {
            handle_connection(stream, &feeds_store_clone, rx).await;
        });
    }
}

async fn handle_connection(stream: TcpStream, feeds_store: &Feeds, mut rx: Receiver<PriceUpdate>) {
    let mut stream_ws = accept_async(stream).await.expect("can't accept TcpStream");

    let subscription: Subscription = match stream_ws.next().await {
        Some(Ok(msg)) => serde_json::from_str(msg.to_text().unwrap()).unwrap(),
        Some(Err(e)) => {
            dbg!(e);
            let _ = stream_ws.close(None).await;
            return
        },
        None => {
            let _ = stream_ws.close(None).await;
            return
        }
    };

    feeds_store.add(subscription.ids.clone());

    loop {
        let price_update: PriceUpdate = match rx.recv().await {
            Ok(p) => p,
            Err(e) => {
                dbg!(e);
                continue;
            }
        };

        if subscription.ids.contains(&price_update.price_feed.id) {
            stream_ws.try_next().now_or_never();
            match stream_ws
                .send(Message::Text(serde_json::to_string(&price_update).unwrap()))
                .await
            {
                Ok(()) => {}
                Err(e) => {
                    dbg!(e);
                    let _ = stream_ws.close(None).await;
                    break;
                }
            };
        }
    }

    feeds_store.remove(&subscription.ids);
}

async fn hermes_stream(tx: Sender<PriceUpdate>, feeds_store: &Feeds) {
    loop {
        while feeds_store.ids.lock().unwrap().is_empty() {
            sleep(Duration::from_secs(1)).await;
        }

        feeds_store.modified.store(false, Ordering::SeqCst);

        let (mut stream, response) = match connect_async(HERMES_WSS_URL).await {
            Ok((s, r)) => (s, r),
            Err(e) => {
                dbg!(e);
                sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        if response.status().as_u16() != 101 {
            continue;
        }
        
        println!("Connected to Hermes");

        let mut feeds_unique: HashSet<H256> = HashSet::new();

        match feeds_store.ids.lock() {
            Ok(feeds) => {
                for feed in feeds.clone().into_iter().flatten() {
                    feeds_unique.insert(feed);
                }
            },
            _ => {}
        };

        let subscription: Subscription = Subscription {
            ids: feeds_unique.drain().collect(),
            r#type: "subscribe".to_string(),
            verbose: false,
            binary: true
        };
        
        let response = stream
            .send(Message::Text(serde_json::to_string(&subscription).unwrap()))
            .await;
        
        if response.is_err() {
            continue;
        }

        stream.next().await;
        
        stream.next().await;

        while let Some(Ok(msg)) = stream.next().await {
            if msg.is_ping() || msg.is_close() {
                continue;
            }
            
            let price_feed_update: PriceUpdate = match serde_json::from_str(msg.to_text().unwrap())
            {
                Ok(p) => p,
                Err(e) => {
                    dbg!(e);
                    dbg!(msg);
                    continue;
                }
            };

            let _ = tx.send(price_feed_update);

            if feeds_store.modified.load(Ordering::SeqCst) {
                println!("Reconnecting to Hermes with updated feeds, if any");
                break;
            }
        }
    }
}
