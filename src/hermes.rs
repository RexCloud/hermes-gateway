use futures_util::{SinkExt, StreamExt};
use primitive_types::H256;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
};
use tokio::{
    sync::broadcast::{Receiver, Sender},
    time::{sleep, Duration},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info, warn};

#[derive(Debug, Deserialize, Serialize)]
pub struct Subscription {
    pub ids: Vec<H256>,
    r#type: String,
    verbose: bool,
    binary: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Price {
    conf: String,
    expo: i32,
    price: String,
    publish_time: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PriceFeed {
    ema_price: Price,
    pub id: H256,
    price: Price,
    vaa: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PriceUpdate {
    r#type: String,
    pub price_feed: PriceFeed,
}

#[derive(Default)]
pub struct Feeds {
    ids: Mutex<Vec<Vec<H256>>>,
    modified: AtomicBool,
}

impl Feeds {
    pub fn add(&self, ids: Vec<H256>) {
        self.ids.lock().unwrap().push(ids);

        self.modified.store(true, Ordering::SeqCst);
    }

    pub fn remove(&self, ids: &Vec<H256>) {
        let mut feeds_lock = self.ids.lock().unwrap();

        match feeds_lock.iter().position(|feeds| *feeds == *ids) {
            Some(index) => {
                feeds_lock.remove(index);
                self.modified.store(true, Ordering::SeqCst);
            }
            None => {
                error!(?ids, feeds_store = ?feeds_lock, "Couldn't find ids in feeds store");
            }
        }
    }
}

const HERMES_WSS_URL: &str = "wss://hermes.pyth.network/ws";

pub async fn stream(tx: Sender<PriceUpdate>, feeds_store: &Feeds) {
    loop {
        while feeds_store.ids.lock().unwrap().is_empty() {
            sleep(Duration::from_secs(1)).await;
        }

        feeds_store.modified.store(false, Ordering::SeqCst);

        let (mut stream, response) = match connect_async(HERMES_WSS_URL).await {
            Ok((s, r)) => (s, r),
            Err(e) => {
                error!("Couldn't connect to {HERMES_WSS_URL}: {e}");
                sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        if response.status().as_u16() != 101 {
            continue;
        }

        let mut feeds_unique: HashSet<H256> = HashSet::new();

        if let Ok(feeds) = feeds_store.ids.lock() {
            for feed in feeds.clone().into_iter().flatten() {
                feeds_unique.insert(feed);
            }
        }

        let subscription: Subscription = Subscription {
            ids: feeds_unique.drain().collect(),
            r#type: "subscribe".to_string(),
            verbose: false,
            binary: true,
        };

        if let Err(e) = stream
            .send(Message::Text(serde_json::to_string(&subscription).unwrap()))
            .await
        {
            error!("{e}");
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
                Err(_) => {
                    error!("{}", msg.to_text().unwrap());
                    continue;
                }
            };

            let _ = tx.send(price_feed_update);

            if feeds_store.modified.load(Ordering::SeqCst) {
                info!("Feeds store updated");
                break;
            }
        }

        info!("Reconnecting to Hermes");
    }
}

pub async fn recv(rx: &mut Receiver<PriceUpdate>) -> PriceUpdate {
    loop {
        match rx.recv().await {
            Ok(p) => p,
            Err(e) => {
                warn!("{e}");
                continue;
            }
        };
    }
}
