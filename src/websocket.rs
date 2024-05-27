use futures_util::{FutureExt, SinkExt, StreamExt, TryStreamExt};
use serde_json;
use tokio::{net::TcpStream, sync::broadcast::Receiver};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tracing::error;

use crate::hermes::{recv, Feeds, PriceUpdate, Subscription};

pub async fn handle_connection(
    stream: TcpStream,
    feeds_store: &Feeds,
    mut rx: Receiver<PriceUpdate>,
) {
    let mut stream_ws = accept_async(stream).await.expect("can't accept TcpStream");

    let subscription: Subscription = match stream_ws.next().await {
        Some(Ok(msg)) => serde_json::from_str(msg.to_text().unwrap()).unwrap(),
        Some(Err(e)) => {
            error!("{e}");
            let _ = stream_ws.close(None).await;
            return;
        }
        None => {
            let _ = stream_ws.close(None).await;
            return;
        }
    };

    feeds_store.add(subscription.ids.clone());

    loop {
        let price_update: PriceUpdate = recv(&mut rx).await;

        if subscription.ids.contains(&price_update.price_feed.id) {
            stream_ws.try_next().now_or_never();

            if stream_ws
                .send(Message::Text(serde_json::to_string(&price_update).unwrap()))
                .await
                .is_err()
            {
                let _ = stream_ws.close(None).await;
                break;
            }
        }
    }

    feeds_store.remove(&subscription.ids);
}
