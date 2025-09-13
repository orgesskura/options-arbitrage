use ordered_float::OrderedFloat;
use rust_decimal::Decimal;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct OrderLevel {
    pub price: f64,
    pub quantity: f64,
}

#[derive(Debug, Clone)]
pub enum OrderBookUpdate {
    Bids {
        exchange: Exchange,
        symbol: String,
        levels: Vec<OrderLevel>,
    },
    Asks {
        exchange: Exchange,
        symbol: String,
        levels: Vec<OrderLevel>,
    },
    ConnectionError {
        exchange: Exchange,
        error: String,
    },
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Exchange {
    Okex,
    Deribit,
}

impl std::fmt::Display for Exchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Exchange::Okex => write!(f, "Okex"),
            Exchange::Deribit => write!(f, "Deribit"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderBook {
    pub bids: BTreeMap<OrderedFloat<f64>, f64>,
    pub asks: BTreeMap<OrderedFloat<f64>, f64>,
    pub symbol: String,
    pub exchange: Exchange,
}

impl OrderBook {
    pub fn new(symbol: String, exchange: Exchange) -> Self {
        Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            symbol,
            exchange,
        }
    }

    pub fn update_bids(&mut self, levels: Vec<OrderLevel>) {
        for level in levels {
            if level.quantity == 0.0 {
                self.bids.remove(&OrderedFloat(level.price));
            } else {
                self.bids.insert(OrderedFloat(level.price), level.quantity);
            }
        }
    }

    pub fn update_asks(&mut self, levels: Vec<OrderLevel>) {
        for level in levels {
            if level.quantity == 0.0 {
                self.asks.remove(&OrderedFloat(level.price));
            } else {
                self.asks.insert(OrderedFloat(level.price), level.quantity);
            }
        }
    }

    pub fn best_bid(&self) -> Option<OrderLevel> {
        self.bids.iter().next_back().map(|(p, &q)| OrderLevel {
            price: p.0,
            quantity: q,
        })
    }

    pub fn best_ask(&self) -> Option<OrderLevel> {
        self.asks.iter().next().map(|(p, &q)| OrderLevel {
            price: p.0,
            quantity: q,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TradeLevel {
    pub buy_price: Decimal,
    pub sell_price: Decimal,
    pub quantity: Decimal,
    pub profit: Decimal,
}

#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    pub buy_exchange: Exchange,
    pub sell_exchange: Exchange,
    pub symbol: String,
    pub trades: Vec<TradeLevel>,
    pub total_profit: Decimal,
    pub total_volume: Decimal,
}

impl ArbitrageOpportunity {
    pub fn show_arb_stats(&self) {
        println!(
            "\nARBITRAGE OPPORTUNITY DETECTED for instrument: {}",
            self.symbol
        );
        println!(
            "Strategy: Buy on {} -> Sell on {}",
            self.buy_exchange, self.sell_exchange
        );

        println!("EXECUTION SEQUENCE:");
        for (i, trade) in self.trades.iter().enumerate() {
            println!(
                "{}. Place BUY order: {} contracts at {} on {}",
                i + 1,
                trade.quantity,
                trade.buy_price,
                self.buy_exchange
            );
            println!(
                "Place SELL order: {} contracts at {} on {}",
                trade.quantity, trade.sell_price, self.sell_exchange
            );
            println!(
                "-> Level Profit: {} (Margin: {})",
                trade.profit,
                trade.sell_price - trade.buy_price
            );
        }

        println!("SUMMARY:");
        println!("Total Volume: {} contracts", self.total_volume);
        println!("Total Profit: {}", self.total_profit);
        println!("{}", "=".repeat(60));
    }
}

pub struct ArbitrageDetector;

impl ArbitrageDetector {
    pub fn detect_arbitrage(
        book_a: &OrderBook,
        book_b: &OrderBook,
    ) -> Option<ArbitrageOpportunity> {
        // Try buy on B, sell on A
        if let Some(opportunity) =
            Self::check_direction(book_a, book_b, &book_b.exchange, &book_a.exchange)
        {
            return Some(opportunity);
        }

        // Try buy on A, sell on B
        if let Some(opportunity) =
            Self::check_direction(book_b, book_a, &book_a.exchange, &book_b.exchange)
        {
            return Some(opportunity);
        }

        None
    }

    fn check_direction(
        sell_book: &OrderBook,
        buy_book: &OrderBook,
        buy_exchange: &Exchange,
        sell_exchange: &Exchange,
    ) -> Option<ArbitrageOpportunity> {
        let best_bid = sell_book.best_bid()?;
        let best_ask = buy_book.best_ask()?;

        if best_bid.price <= best_ask.price {
            return None;
        }

        let mut trades = Vec::new();
        let mut total_profit = Decimal::ZERO;
        let mut total_volume = Decimal::ZERO;

        let mut sell_iter = sell_book.bids.iter().rev();
        let mut buy_iter = buy_book.asks.iter();

        let mut current_sell = sell_iter.next().map(|(p, &q)| (p.0, q));
        let mut current_buy = buy_iter.next().map(|(p, &q)| (p.0, q));
        let mut remaining_sell_qty = 0.0;
        let mut remaining_buy_qty = 0.0;

        while let (Some((sell_price, sell_qty)), Some((buy_price, buy_qty))) =
            (current_sell, current_buy)
        {
            if sell_price <= buy_price {
                break;
            }

            let available_sell_qty = if remaining_sell_qty > 0.0 {
                remaining_sell_qty
            } else {
                sell_qty
            };
            let available_buy_qty = if remaining_buy_qty > 0.0 {
                remaining_buy_qty
            } else {
                buy_qty
            };

            let sell_price_d = Decimal::try_from(sell_price).ok()?;
            let buy_price_d = Decimal::try_from(buy_price).ok()?;
            let trade_qty_f64 = available_sell_qty.min(available_buy_qty);
            let trade_qty = Decimal::try_from(trade_qty_f64).ok()?;

            let profit = trade_qty * (sell_price_d - buy_price_d);

            trades.push(TradeLevel {
                buy_price: buy_price_d,
                sell_price: sell_price_d,
                quantity: trade_qty,
                profit,
            });

            total_profit += profit;
            total_volume += trade_qty;

            match available_sell_qty.partial_cmp(&available_buy_qty).unwrap() {
                std::cmp::Ordering::Less => {
                    current_sell = sell_iter.next().map(|(p, &q)| (p.0, q));
                    remaining_sell_qty = 0.0;
                    remaining_buy_qty = available_buy_qty - available_sell_qty;
                }
                std::cmp::Ordering::Greater => {
                    current_buy = buy_iter.next().map(|(p, &q)| (p.0, q));
                    remaining_buy_qty = 0.0;
                    remaining_sell_qty = available_sell_qty - available_buy_qty;
                }
                std::cmp::Ordering::Equal => {
                    current_sell = sell_iter.next().map(|(p, &q)| (p.0, q));
                    current_buy = buy_iter.next().map(|(p, &q)| (p.0, q));
                    remaining_sell_qty = 0.0;
                    remaining_buy_qty = 0.0;
                }
            }
        }

        if total_profit > Decimal::ZERO {
            Some(ArbitrageOpportunity {
                buy_exchange: buy_exchange.clone(),
                sell_exchange: sell_exchange.clone(),
                symbol: sell_book.symbol.clone(),
                trades,
                total_profit,
                total_volume,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;

    #[test]
    fn test_simple_single_level_arbitrage() {
        let mut okex_book = OrderBook::new("BTC-USD-240427-56000-C".to_string(), Exchange::Okex);
        let mut deribit_book = OrderBook::new("BTC-27APR24-56000-C".to_string(), Exchange::Deribit);

        deribit_book.update_bids(vec![OrderLevel {
            price: 0.150,
            quantity: 100.0,
        }]);

        okex_book.update_asks(vec![OrderLevel {
            price: 0.140,
            quantity: 100.0,
        }]);

        let opportunity = ArbitrageDetector::detect_arbitrage(&okex_book, &deribit_book).unwrap();

        assert_eq!(opportunity.trades.len(), 1);
        assert_eq!(opportunity.total_volume, dec!(100.0));
        assert_eq!(
            opportunity.total_profit,
            dec!(100.0) * (dec!(0.150) - dec!(0.140))
        );
    }

    #[test]
    fn test_multi_level_profit_accumulation() {
        let mut okex_book = OrderBook::new("BTC-USD-240427-56000-C".to_string(), Exchange::Okex);
        let mut deribit_book = OrderBook::new("BTC-27APR24-56000-C".to_string(), Exchange::Deribit);

        okex_book.update_bids(vec![
            OrderLevel {
                price: 0.150,
                quantity: 50.0,
            },
            OrderLevel {
                price: 0.145,
                quantity: 75.0,
            },
            OrderLevel {
                price: 0.140,
                quantity: 100.0,
            },
        ]);

        deribit_book.update_asks(vec![
            OrderLevel {
                price: 0.135,
                quantity: 30.0,
            },
            OrderLevel {
                price: 0.138,
                quantity: 40.0,
            },
            OrderLevel {
                price: 0.142,
                quantity: 200.0,
            },
        ]);

        let opportunity = ArbitrageDetector::detect_arbitrage(&okex_book, &deribit_book).unwrap();
        assert_eq!(opportunity.trades.len(), 4);

        let expected_profit = dec!(30.0) * (dec!(0.150) - dec!(0.135))
            + dec!(20.0) * (dec!(0.150) - dec!(0.138))
            + dec!(20.0) * (dec!(0.145) - dec!(0.138))
            + dec!(55.0) * (dec!(0.145) - dec!(0.142));

        assert_eq!(opportunity.total_profit, expected_profit);
        assert_eq!(opportunity.total_volume, dec!(125.0));
    }

    #[test]
    fn test_partial_level_consumption_complex() {
        let mut okex_book = OrderBook::new("TEST".to_string(), Exchange::Okex);
        let mut deribit_book = OrderBook::new("TEST".to_string(), Exchange::Deribit);

        okex_book.update_bids(vec![
            OrderLevel {
                price: 0.200,
                quantity: 25.0,
            }, // Small bid
            OrderLevel {
                price: 0.190,
                quantity: 1000.0,
            }, // Large bid
            OrderLevel {
                price: 0.180,
                quantity: 500.0,
            }, // Larger bid
        ]);

        deribit_book.update_asks(vec![
            OrderLevel {
                price: 0.170,
                quantity: 100.0,
            }, // Medium ask
            OrderLevel {
                price: 0.175,
                quantity: 200.0,
            }, // Larger ask
            OrderLevel {
                price: 0.185,
                quantity: 2000.0,
            }, // Very large ask
        ]);

        let opportunity = ArbitrageDetector::detect_arbitrage(&okex_book, &deribit_book).unwrap();

        // Expected trades:
        // 1. Buy 25 at 0.170, Sell at 0.200 = 25 * 0.030 = 0.75
        // 2. Buy 75 at 0.170, Sell at 0.190 = 75 * 0.020 = 1.50
        // 3. Buy 200 at 0.175, Sell at 0.190 = 200 * 0.015 = 3.00
        // 4. Buy remaining at 0.185 vs 0.190 = some amount * 0.005

        let expected_volume = dec!(25.0) + dec!(75.0) + dec!(200.0); // At least 300
        let expected_min_profit =
            dec!(25.0) * dec!(0.030) + dec!(75.0) * dec!(0.020) + dec!(200.0) * dec!(0.015);

        assert!(opportunity.total_volume >= expected_volume);
        assert!(opportunity.total_profit >= expected_min_profit);
        assert!(opportunity.trades.len() >= 3);
    }

    #[test]
    fn test_exactly_matching_quantities() {
        let mut okex_book = OrderBook::new("TEST".to_string(), Exchange::Okex);
        let mut deribit_book = OrderBook::new("TEST".to_string(), Exchange::Deribit);

        // Scenario where quantities match exactly across multiple levels
        okex_book.update_bids(vec![
            OrderLevel {
                price: 0.160,
                quantity: 75.0,
            },
            OrderLevel {
                price: 0.150,
                quantity: 100.0,
            },
            OrderLevel {
                price: 0.145,
                quantity: 50.0,
            },
        ]);

        deribit_book.update_asks(vec![
            OrderLevel {
                price: 0.140,
                quantity: 50.0,
            },
            OrderLevel {
                price: 0.135,
                quantity: 100.0,
            },
            OrderLevel {
                price: 0.130,
                quantity: 75.0,
            },
        ]);

        let opportunity = ArbitrageDetector::detect_arbitrage(&okex_book, &deribit_book).unwrap();

        // Should have exactly 3 trades with perfect quantity matches
        assert_eq!(opportunity.trades.len(), 3);

        let expected_profit = dec!(75.0) * (dec!(0.160) - dec!(0.130))
            + dec!(100.0) * (dec!(0.150) - dec!(0.135))
            + dec!(50.0) * (dec!(0.145) - dec!(0.140));

        assert_eq!(opportunity.total_volume, dec!(225.0));
        assert_eq!(opportunity.total_profit, expected_profit);
    }

    #[test]
    fn test_deep_order_book_many_levels() {
        let mut okex_book = OrderBook::new("DEEP-TEST".to_string(), Exchange::Okex);
        let mut deribit_book = OrderBook::new("DEEP-TEST".to_string(), Exchange::Deribit);

        // Deep order book with many small levels
        okex_book.update_bids(vec![
            OrderLevel {
                price: 0.200,
                quantity: 10.0,
            },
            OrderLevel {
                price: 0.199,
                quantity: 15.0,
            },
            OrderLevel {
                price: 0.198,
                quantity: 20.0,
            },
            OrderLevel {
                price: 0.197,
                quantity: 25.0,
            },
            OrderLevel {
                price: 0.196,
                quantity: 30.0,
            },
            OrderLevel {
                price: 0.195,
                quantity: 35.0,
            },
            OrderLevel {
                price: 0.194,
                quantity: 40.0,
            },
            OrderLevel {
                price: 0.193,
                quantity: 45.0,
            },
        ]);

        deribit_book.update_asks(vec![
            OrderLevel {
                price: 0.185,
                quantity: 50.0,
            },
            OrderLevel {
                price: 0.186,
                quantity: 45.0,
            },
            OrderLevel {
                price: 0.187,
                quantity: 40.0,
            },
            OrderLevel {
                price: 0.188,
                quantity: 35.0,
            },
            OrderLevel {
                price: 0.189,
                quantity: 30.0,
            },
            OrderLevel {
                price: 0.190,
                quantity: 25.0,
            },
            OrderLevel {
                price: 0.191,
                quantity: 20.0,
            },
            OrderLevel {
                price: 0.192,
                quantity: 15.0,
            },
        ]);

        let opportunity = ArbitrageDetector::detect_arbitrage(&okex_book, &deribit_book).unwrap();

        // Should traverse multiple levels
        assert!(opportunity.trades.len() >= 5);
        assert!(opportunity.total_volume > dec!(100.0));
        assert!(opportunity.total_profit > dec!(1.0));
    }

    #[test]
    fn test_profit_precision_accumulation() {
        let mut okex_book = OrderBook::new("PRECISION-TEST".to_string(), Exchange::Okex);
        let mut deribit_book = OrderBook::new("PRECISION-TEST".to_string(), Exchange::Deribit);

        // Test precision with small price differences
        okex_book.update_bids(vec![
            OrderLevel {
                price: 0.123456789,
                quantity: 1000000.0,
            },
            OrderLevel {
                price: 0.123456788,
                quantity: 2000000.0,
            },
            OrderLevel {
                price: 0.123456787,
                quantity: 1500000.0,
            },
        ]);

        deribit_book.update_asks(vec![
            OrderLevel {
                price: 0.123456785,
                quantity: 500000.0,
            },
            OrderLevel {
                price: 0.123456786,
                quantity: 1000000.0,
            },
            OrderLevel {
                price: 0.123456787,
                quantity: 3000000.0,
            },
        ]);

        let opportunity = ArbitrageDetector::detect_arbitrage(&okex_book, &deribit_book).unwrap();

        // Verify that small decimal differences are handled correctly
        for (i, trade) in opportunity.trades.iter().enumerate() {
            println!(
                "Trade {}: {} * ({} - {}) = {}",
                i + 1,
                trade.quantity,
                trade.sell_price,
                trade.buy_price,
                trade.profit
            );
        }

        assert!(opportunity.total_profit > Decimal::ZERO);
        assert!(opportunity.total_volume > dec!(500000.0));

        // Ensure precision is maintained - should not lose decimal places
        assert!(opportunity.total_profit.to_string().contains('.'));
    }

    #[test]
    fn test_no_arbitrage_scenarios() {
        // Scenario 1: Normal spread (no crossing)
        let mut okex_book = OrderBook::new("NO-ARB-1".to_string(), Exchange::Okex);
        let mut deribit_book = OrderBook::new("NO-ARB-1".to_string(), Exchange::Deribit);
        okex_book.update_bids(vec![OrderLevel {
            price: 0.130,
            quantity: 100.0,
        }]);
        okex_book.update_asks(vec![OrderLevel {
            price: 0.135,
            quantity: 100.0,
        }]);
        deribit_book.update_bids(vec![OrderLevel {
            price: 0.129,
            quantity: 100.0,
        }]);
        deribit_book.update_asks(vec![OrderLevel {
            price: 0.136,
            quantity: 100.0,
        }]);
        assert!(ArbitrageDetector::detect_arbitrage(&okex_book, &deribit_book).is_none());

        // Scenario 2: Equal prices
        let mut okex_book2 = OrderBook::new("NO-ARB-2".to_string(), Exchange::Okex);
        let mut deribit_book2 = OrderBook::new("NO-ARB-2".to_string(), Exchange::Deribit);
        okex_book2.update_bids(vec![OrderLevel {
            price: 0.150,
            quantity: 100.0,
        }]);
        deribit_book2.update_asks(vec![OrderLevel {
            price: 0.150,
            quantity: 100.0,
        }]);
        assert!(ArbitrageDetector::detect_arbitrage(&okex_book2, &deribit_book2).is_none());

        // Scenario 3: Empty order books
        let okex_book3 = OrderBook::new("NO-ARB-3".to_string(), Exchange::Okex);
        let deribit_book3 = OrderBook::new("NO-ARB-3".to_string(), Exchange::Deribit);
        assert!(ArbitrageDetector::detect_arbitrage(&okex_book3, &deribit_book3).is_none());
    }

    #[test]
    fn test_edge_case_scenarios() {
        // Edge Case 1: Zero quantities
        let mut okex_book = OrderBook::new("EDGE-1".to_string(), Exchange::Okex);
        let mut deribit_book = OrderBook::new("EDGE-1".to_string(), Exchange::Deribit);
        okex_book.update_bids(vec![OrderLevel {
            price: 0.160,
            quantity: 0.0,
        }]);
        deribit_book.update_asks(vec![OrderLevel {
            price: 0.140,
            quantity: 100.0,
        }]);
        let opportunity = ArbitrageDetector::detect_arbitrage(&okex_book, &deribit_book);
        assert!(opportunity.is_none());

        // Edge Case 2: Very small quantities
        let mut okex_book2 = OrderBook::new("EDGE-2".to_string(), Exchange::Okex);
        let mut deribit_book2 = OrderBook::new("EDGE-2".to_string(), Exchange::Deribit);
        okex_book2.update_bids(vec![OrderLevel {
            price: 0.160,
            quantity: 0.001,
        }]);
        deribit_book2.update_asks(vec![OrderLevel {
            price: 0.140,
            quantity: 0.001,
        }]);
        let opportunity2 =
            ArbitrageDetector::detect_arbitrage(&okex_book2, &deribit_book2).unwrap();
        assert!(opportunity2.total_profit > Decimal::ZERO);
        assert_eq!(opportunity2.total_volume, dec!(0.001));
    }
}
