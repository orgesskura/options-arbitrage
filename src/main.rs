mod exchanges;
mod orderbook;
mod parsing_utils;

use crate::{
    exchanges::{deribit_websocket_task, okex_websocket_task},
    orderbook::{ArbitrageDetector, Exchange, OrderBook, OrderBookUpdate},
};
use anyhow::Result;
use clap::Parser;
use parsing_utils::InstrumentValidator;
use std::collections::HashMap;
use tokio::sync::mpsc;

#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    #[arg(long, required = true)]
    okex_symbol: String,
    #[arg(long, required = true)]
    deribit_symbol: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let okex_symbol = args.okex_symbol;
    let deribit_symbol = args.deribit_symbol;

    println!(
        "LET'S GOOO: Trying to find arbitrage between {okex_symbol} (Okex) and {deribit_symbol} \
         (Deribit)"
    );

    match InstrumentValidator::are_same_instrument(&okex_symbol, &deribit_symbol) {
        Ok(true) => {}
        Ok(false) => {
            eprintln!("Error: Instruments do not match!");
            eprintln!("Okex: {okex_symbol}");
            eprintln!("Deribit: {deribit_symbol}");
            return Ok(());
        }
        Err(e) => {
            eprintln!("Failed to parse instruments: {e}");
            return Ok(());
        }
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<OrderBookUpdate>();

    tokio::spawn({
        let symbol = okex_symbol.clone();
        let tx = tx.clone();
        async move { okex_websocket_task(symbol, tx).await }
    });

    tokio::spawn({
        let symbol = deribit_symbol.clone();
        let tx = tx.clone();
        async move { deribit_websocket_task(symbol, tx).await }
    });

    let mut books = HashMap::new();
    let mut last_fingerprint = None;

    while let Some(update) = rx.recv().await {
        match update {
            OrderBookUpdate::Bids {
                exchange,
                symbol,
                levels,
            } => {
                let book = books
                    .entry(exchange.clone())
                    .or_insert_with(|| OrderBook::new(symbol, exchange));
                book.update_bids(levels);
            }
            OrderBookUpdate::Asks {
                exchange,
                symbol,
                levels,
            } => {
                let book = books
                    .entry(exchange.clone())
                    .or_insert_with(|| OrderBook::new(symbol, exchange));
                book.update_asks(levels);
            }
            OrderBookUpdate::ConnectionError { exchange, error } => {
                eprintln!("Connection error from {exchange}: {error}");
            }
        }

        if let (Some(okex), Some(deribit)) =
            (books.get(&Exchange::Okex), books.get(&Exchange::Deribit))
        {
            if let Some(opp) = ArbitrageDetector::detect_arbitrage(okex, deribit) {
                // Only print arbitrage opportunities when new opportunity is spotted.
                let fp = (opp.symbol.clone(), opp.total_profit);
                if Some(fp.clone()) != last_fingerprint {
                    opp.show_arb_stats();
                    last_fingerprint = Some(fp);
                }
            }
        }
    }

    Ok(())
}
