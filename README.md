# B2C2 Rust Arbitrage Exercise

This project connects to **Okex** and **Deribit** websockets, builds live order books, and detects arbitrage opportunities between the two exchanges for a given instrument.

---

## How it works
- Maintains local order books (`BTreeMap` for bids/asks).
- Updates books from exchange websocket feeds.
- Detects arbitrage by matching bids from one book against asks from the other, traversing multiple levels if profitable.
- Prints execution sequence and profit summary when an opportunity appears.
- Handles websocket disconnects with automatic reconnect + backoff.
- Includes a test suite covering:
  - Single-level and multi-level arbitrage
  - Partial fills
  - Deep order books
  - Precision edge cases
  - No-arbitrage scenarios

---

## Running
```bash
cargo run --release
```

## Testing
```bash
cargo test
```

---

## Sample Output

```
ARBITRAGE OPPORTUNITY DETECTED for instrument: BTC-USD-251031-140000-P
Strategy: Buy on Okex -> Sell on Deribit

EXECUTION SEQUENCE:
1. Place BUY order: 100 contracts at 0.140 on Okex
   Place SELL order: 100 contracts at 0.150 on Deribit
   -> Level Profit: 1.0 (Margin: 0.010)

SUMMARY:
Total Volume: 100 contracts
Total Profit: 1.0
============================================================
```

---

## Potential Improvements
- **Precision**: Use `Decimal` for prices and quantities directly in `OrderLevel` instead of `f64` to avoid float→decimal conversions.
- **Order book state**: Clear/reset on reconnect to prevent stale data.
- **Reconnect strategy**: Smarter exponential backoff with jitter rather than fixed steps.
- **Websocket handling**: Explicitly manage ping/pong and subscription acknowledgment messages.
- **Logging/metrics**: Add structured logging and metrics export for monitoring.
- **Extensibility**: Add support for more exchanges, multiple instruments, and configurable strategies.
- **Execution layer**: Extend beyond detection into real trading like order placing. Currently I do not clear currently observed arbitrages.

---

