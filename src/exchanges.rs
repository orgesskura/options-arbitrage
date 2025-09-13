use crate::orderbook::{Exchange, OrderBookUpdate, OrderLevel};
use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::{
    sync::mpsc,
    time::{Duration, sleep},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const OKEX_PING_INTERVAL_SECS: u64 = 15;

fn parse_okex_levels(levels: Vec<Vec<String>>) -> Vec<OrderLevel> {
    levels
        .into_iter()
        .filter_map(|l| {
            if l.len() >= 2 {
                Some(OrderLevel {
                    price: l[0].parse().ok()?,
                    quantity: l[1].parse().ok()?,
                })
            } else {
                None
            }
        })
        .collect()
}

fn parse_deribit_levels(levels: Vec<(f64, f64)>) -> Vec<OrderLevel> {
    levels
        .into_iter()
        .map(|(p, q)| OrderLevel {
            price: p,
            quantity: q,
        })
        .collect()
}

#[derive(Deserialize, Debug)]
struct OkexResponse {
    data: Vec<OkexOrderBookData>,
}

#[derive(Deserialize, Debug)]
struct OkexOrderBookData {
    asks: Vec<Vec<String>>,
    bids: Vec<Vec<String>>,
}

#[derive(Deserialize, Debug)]
struct DeribitResponse {
    params: DeribitParams,
}

#[derive(Deserialize, Debug)]
struct DeribitParams {
    data: DeribitOrderBookData,
}

#[derive(Deserialize, Debug)]
struct DeribitOrderBookData {
    asks: Vec<(f64, f64)>,
    bids: Vec<(f64, f64)>,
}

pub async fn okex_websocket_task(
    symbol: String,
    tx: mpsc::UnboundedSender<OrderBookUpdate>,
) -> Result<()> {
    let url = "wss://ws.okx.com:8443/ws/v5/public";
    let mut attempt: u32 = 0;

    loop {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                attempt = 0;
                let (mut write, mut read) = ws_stream.split();
                let subscribe_msg = serde_json::json!({
                    "op": "subscribe",
                    "args": [{"channel": "books", "instId": &symbol}]
                });
                if write
                    .send(Message::text(subscribe_msg.to_string()))
                    .await
                    .is_err()
                {
                    continue;
                }
                println!("Okex connected");

                let mut ping_interval =
                    tokio::time::interval(Duration::from_secs(OKEX_PING_INTERVAL_SECS));

                loop {
                    tokio::select! {
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    if let Ok(resp) = serde_json::from_str::<OkexResponse>(&text) {
                                        if let Some(data) = resp.data.first() {
                                            // skip empty updates
                                            if data.bids.is_empty() && data.asks.is_empty() {
                                                continue;
                                            }
                                            let bids = parse_okex_levels(data.bids.clone());
                                            let asks = parse_okex_levels(data.asks.clone());
                                            let _ = tx.send(OrderBookUpdate::Bids {
                                                exchange: Exchange::Okex,
                                                symbol: symbol.clone(),
                                                levels: bids,
                                            });
                                            let _ = tx.send(OrderBookUpdate::Asks {
                                                exchange: Exchange::Okex,
                                                symbol: symbol.clone(),
                                                levels: asks,
                                            });
                                        }
                                    }
                                }
                                Some(Ok(Message::Close(frame))) => {
                                    let reason = frame
                                        .map(|f| f.reason.to_string())
                                        .unwrap_or_else(|| "Connection closed by server".to_string());
                                    let _ = tx.send(OrderBookUpdate::ConnectionError {
                                        exchange: Exchange::Okex,
                                        error: reason,
                                    });
                                    break;
                                }
                                Some(Err(e)) => {
                                    let _ = tx.send(OrderBookUpdate::ConnectionError {
                                        exchange: Exchange::Okex,
                                        error: format!("Websocket error: {e}"),
                                    });
                                    break;
                                }
                                None => break,
                                _ => {}
                            }
                        }
                        _ = ping_interval.tick() => {
                            let _ = write.send(Message::text("ping")).await;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(OrderBookUpdate::ConnectionError {
                    exchange: Exchange::Okex,
                    error: format!("Failed to connect: {e}"),
                });
            }
        }

        attempt += 1;
        let base = ((attempt.min(5)) * 5) as u64;
        let jitter: u64 = rand::random::<u64>() % 5;
        let backoff = base + jitter;
        println!("Okex reconnecting in {backoff}s...");
        sleep(Duration::from_secs(backoff)).await;
    }
}

pub async fn deribit_websocket_task(
    symbol: String,
    tx: mpsc::UnboundedSender<OrderBookUpdate>,
) -> Result<()> {
    let url = "wss://www.deribit.com/ws/api/v2";
    let mut attempt: u32 = 0;

    loop {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                attempt = 0;
                let (mut write, mut read) = ws_stream.split();
                let subscribe_msg = serde_json::json!({
                    "method": "public/subscribe",
                    "params": {"channels": [format!("book.{}.none.20.100ms", symbol)]},
                    "jsonrpc": "2.0",
                    "id": 0
                });
                if write
                    .send(Message::text(subscribe_msg.to_string()))
                    .await
                    .is_err()
                {
                    continue;
                }
                println!("Deribit connected");

                let mut ping_interval = tokio::time::interval(Duration::from_secs(15));

                loop {
                    tokio::select! {
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    if let Ok(resp) = serde_json::from_str::<DeribitResponse>(&text) {
                                        let bids = parse_deribit_levels(resp.params.data.bids);
                                        let asks = parse_deribit_levels(resp.params.data.asks);
                                        let _ = tx.send(OrderBookUpdate::Bids {
                                            exchange: Exchange::Deribit,
                                            symbol: symbol.clone(),
                                            levels: bids,
                                        });
                                        let _ = tx.send(OrderBookUpdate::Asks {
                                            exchange: Exchange::Deribit,
                                            symbol: symbol.clone(),
                                            levels: asks,
                                        });
                                    }
                                }
                                Some(Ok(Message::Close(frame))) => {
                                    let reason = frame
                                        .map(|f| f.reason.to_string())
                                        .unwrap_or_else(|| "Connection closed by server".to_string());
                                    let _ = tx.send(OrderBookUpdate::ConnectionError {
                                        exchange: Exchange::Deribit,
                                        error: reason,
                                    });
                                    break;
                                }
                                Some(Err(e)) => {
                                    let _ = tx.send(OrderBookUpdate::ConnectionError {
                                        exchange: Exchange::Deribit,
                                        error: format!("Websocket error: {e}"),
                                    });
                                    break;
                                }
                                None => break,
                                _ => {}
                            }
                        }
                        _ = ping_interval.tick() => {
                            let heartbeat = serde_json::json!({
                                "id": 42,
                                "method": "public/test",
                                "params": {},
                                "jsonrpc": "2.0"
                            });
                            let _ = write.send(Message::text(heartbeat.to_string())).await;
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(OrderBookUpdate::ConnectionError {
                    exchange: Exchange::Deribit,
                    error: format!("Failed to connect: {e}"),
                });
            }
        }

        attempt += 1;
        let base = ((attempt.min(5)) * 5) as u64;
        let jitter: u64 = rand::random::<u64>() % 5;
        let backoff = base + jitter;
        println!("Deribit reconnecting in {backoff}s...");
        sleep(Duration::from_secs(backoff)).await;
    }
}
