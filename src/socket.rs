use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixStream,
    sync::broadcast::Receiver,
};
use tracing::error;

use crate::hermes::{recv, Feeds, PriceUpdate, Subscription};

pub async fn handle_connection(
    mut stream: UnixStream,
    feeds_store: &Feeds,
    mut rx: Receiver<PriceUpdate>,
) {
    let mut buf: Vec<u8> = vec![0; 4096];

    let subscription: Subscription = match stream.read(&mut buf).await {
        Ok(len) => match serde_json::from_slice(&buf[..len]) {
            Ok(s) => s,
            Err(e) => {
                error!("{e}");
                let _ = stream.shutdown().await;
                return;
            }
        },
        Err(e) => {
            error!("Read failed: {e}");
            let _ = stream.shutdown().await;
            return;
        }
    };

    drop(buf);

    feeds_store.add(subscription.ids.clone());

    loop {
        let price_update: PriceUpdate = recv(&mut rx).await;

        if subscription.ids.contains(&price_update.price_feed.id) {
            let data: Vec<u8> = serde_json::to_vec(&price_update).unwrap();

            let len: [u8; 2] = (data.len() as u16).to_be_bytes();

            if stream.write_all(&[&len[..], &data].concat()).await.is_err() {
                let _ = stream.shutdown().await;
                break;
            }
        }
    }

    feeds_store.remove(&subscription.ids);
}
