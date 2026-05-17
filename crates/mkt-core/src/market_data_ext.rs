use async_trait::async_trait;
use std::collections::HashMap;

use mkt_types::{Kline, KlineRequest, LastPrice, MarketInfo, Symbol};

use crate::{MarketData, Result};

#[async_trait]
pub trait MarketDataExt: MarketData {
    async fn markets_by_symbol(&self) -> Result<HashMap<Symbol, MarketInfo>> {
        Ok(self
            .markets()
            .await?
            .into_iter()
            .map(|market| (market.symbol.clone(), market))
            .collect())
    }

    async fn last_prices_by_symbol(
        &self,
        symbols: Option<&[Symbol]>,
    ) -> Result<HashMap<Symbol, LastPrice>> {
        Ok(self
            .last_prices(symbols)
            .await?
            .into_iter()
            .map(|price| (price.symbol.clone(), price))
            .collect())
    }

    async fn kline_history(&self, request: KlineRequest) -> Result<Vec<Kline>> {
        let Some(limit) = request.limit else {
            return self.klines(request).await;
        };
        if limit == 0 {
            return self.klines(request).await;
        }

        let start = request.start;
        let end = request.end;
        let mut request = request;
        let mut previous_start = request.start;
        let mut history = Vec::new();

        loop {
            let batch = self.klines(request.clone()).await?;
            if batch.is_empty() {
                break;
            }

            let batch_len = batch.len();
            let next_start = batch.last().map(|kline| kline.open_time);
            history = Kline::merge(history, batch);

            let Some(next_start) = next_start else {
                break;
            };
            if batch_len < limit as usize || Some(next_start) == previous_start {
                break;
            }

            previous_start = Some(next_start);
            request.start = Some(next_start);
        }

        Ok(Kline::window(history, start, end))
    }
}

impl<T> MarketDataExt for T where T: MarketData + ?Sized {}

#[cfg(test)]
mod tests {
    use super::MarketDataExt;
    use crate::{MarketData, Result};
    use async_trait::async_trait;
    use mkt_types::{
        Decimal, ExchangeId, Kline, KlineInterval, KlineRequest, KnownExchange, LastPrice,
        MarketInfo, MarketStatus, OrderBook, Symbol, Trade, TradingConstraints, TradingPermissions,
    };
    use time::OffsetDateTime;

    struct TestMarketData {
        markets: Vec<MarketInfo>,
        prices: Vec<LastPrice>,
    }

    #[async_trait]
    impl MarketData for TestMarketData {
        async fn markets(&self) -> Result<Vec<MarketInfo>> {
            Ok(self.markets.clone())
        }

        async fn market(&self, symbol: &Symbol) -> Result<Option<MarketInfo>> {
            Ok(self
                .markets
                .iter()
                .rev()
                .find(|market| market.symbol == *symbol)
                .cloned())
        }

        async fn last_price(&self, symbol: &Symbol) -> Result<LastPrice> {
            Ok(self
                .prices
                .iter()
                .find(|price| price.symbol == *symbol)
                .cloned()
                .expect("test price must exist"))
        }

        async fn last_prices(&self, symbols: Option<&[Symbol]>) -> Result<Vec<LastPrice>> {
            Ok(match symbols {
                Some(symbols) => self
                    .prices
                    .iter()
                    .filter(|price| symbols.contains(&price.symbol))
                    .cloned()
                    .collect(),
                None => self.prices.clone(),
            })
        }

        async fn order_book(&self, _symbol: &Symbol, _depth: Option<u16>) -> Result<OrderBook> {
            unimplemented!("order_book is not needed in this test")
        }

        async fn recent_trades(&self, _symbol: &Symbol, _limit: Option<u32>) -> Result<Vec<Trade>> {
            unimplemented!("recent_trades is not needed in this test")
        }

        async fn klines(&self, _request: KlineRequest) -> Result<Vec<Kline>> {
            unimplemented!("klines is not needed in this test")
        }
    }

    fn timestamp(value: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(value).expect("test timestamp must be valid")
    }

    fn market(symbol: &str, status: MarketStatus) -> MarketInfo {
        MarketInfo::builder()
            .exchange_id(ExchangeId::from(KnownExchange::Binance))
            .symbol(Symbol::spot(symbol))
            .status(status)
            .base_asset("BASE")
            .quote_asset("QUOTE")
            .trading_permissions(TradingPermissions::default())
            .trading_constraints(TradingConstraints::default())
            .build()
            .expect("market must build")
    }

    fn price(symbol: &str, value: i64) -> LastPrice {
        LastPrice::new(Symbol::spot(symbol), Decimal::new(value, 0))
    }

    #[tokio::test]
    async fn market_data_helpers_build_symbol_maps_with_last_value_wins() {
        let btc = Symbol::spot("BTCUSDT");
        let eth = Symbol::spot("ETHUSDT");
        let market_data = TestMarketData {
            markets: vec![
                market("BTCUSDT", MarketStatus::Trading),
                market("ETHUSDT", MarketStatus::Trading),
                market("BTCUSDT", MarketStatus::Halted),
            ],
            prices: vec![
                price("BTCUSDT", 100),
                price("ETHUSDT", 200),
                price("BTCUSDT", 101),
            ],
        };

        let markets = market_data
            .markets_by_symbol()
            .await
            .expect("markets map must build");
        assert_eq!(markets.len(), 2);
        assert_eq!(
            markets.get(&btc).expect("btc market must exist").status,
            MarketStatus::Halted
        );
        assert_eq!(
            market_data
                .market(&btc)
                .await
                .expect("market lookup must succeed")
                .expect("btc market must exist")
                .status,
            MarketStatus::Halted
        );

        let prices = market_data
            .last_prices_by_symbol(Some(&[btc.clone(), eth.clone()]))
            .await
            .expect("prices map must build");
        assert_eq!(prices.len(), 2);
        assert_eq!(
            prices.get(&btc).expect("btc price must exist").price,
            Decimal::new(101, 0)
        );
        assert_eq!(
            prices.get(&eth).expect("eth price must exist").price,
            Decimal::new(200, 0)
        );
    }

    fn kline_at(day: u8) -> Kline {
        let open_time = match day {
            1 => timestamp(1_767_225_600),
            2 => timestamp(1_767_312_000),
            3 => timestamp(1_767_398_400),
            _ => timestamp(1_767_484_800),
        };
        let close_time = match day {
            1 => timestamp(1_767_312_000),
            2 => timestamp(1_767_398_400),
            3 => timestamp(1_767_484_800),
            _ => timestamp(1_767_571_200),
        };

        Kline::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .interval(KlineInterval::D1)
            .open_time(open_time)
            .close_time(close_time)
            .open(Decimal::new(100 + i64::from(day), 0))
            .high(Decimal::new(110 + i64::from(day), 0))
            .low(Decimal::new(90 + i64::from(day), 0))
            .close(Decimal::new(105 + i64::from(day), 0))
            .volume_base(Decimal::ONE)
            .closed(true)
            .build()
            .expect("kline must build")
    }

    #[tokio::test]
    async fn kline_history_pages_without_execution_state() {
        struct TestHistoryMarketData {
            klines: Vec<Kline>,
        }

        #[async_trait]
        impl MarketData for TestHistoryMarketData {
            async fn markets(&self) -> Result<Vec<MarketInfo>> {
                unimplemented!("markets is not needed in this test")
            }

            async fn market(&self, _symbol: &Symbol) -> Result<Option<MarketInfo>> {
                unimplemented!("market is not needed in this test")
            }

            async fn last_price(&self, _symbol: &Symbol) -> Result<LastPrice> {
                unimplemented!("last_price is not needed in this test")
            }

            async fn last_prices(&self, _symbols: Option<&[Symbol]>) -> Result<Vec<LastPrice>> {
                unimplemented!("last_prices is not needed in this test")
            }

            async fn order_book(&self, _symbol: &Symbol, _depth: Option<u16>) -> Result<OrderBook> {
                unimplemented!("order_book is not needed in this test")
            }

            async fn recent_trades(
                &self,
                _symbol: &Symbol,
                _limit: Option<u32>,
            ) -> Result<Vec<Trade>> {
                unimplemented!("recent_trades is not needed in this test")
            }

            async fn klines(&self, request: KlineRequest) -> Result<Vec<Kline>> {
                let start = request.start;
                let end = request.end;
                let limit = request.limit.unwrap_or(u32::MAX) as usize;

                Ok(self
                    .klines
                    .iter()
                    .filter(|kline| start.is_none_or(|start| kline.open_time >= start))
                    .filter(|kline| end.is_none_or(|end| kline.open_time < end))
                    .take(limit)
                    .cloned()
                    .collect())
            }
        }

        let market_data = TestHistoryMarketData {
            klines: vec![kline_at(1), kline_at(2), kline_at(3)],
        };
        let history = market_data
            .kline_history(
                KlineRequest::builder()
                    .symbol(Symbol::spot("BTCUSDT"))
                    .interval(KlineInterval::D1)
                    .start(Some(timestamp(1_767_225_600)))
                    .end(Some(timestamp(1_767_484_800)))
                    .limit(Some(2))
                    .build()
                    .expect("request must build"),
            )
            .await
            .expect("history must build");

        assert_eq!(history.len(), 3);
        assert_eq!(history[0].open_time, timestamp(1_767_225_600));
        assert_eq!(history[1].open_time, timestamp(1_767_312_000));
        assert_eq!(history[2].open_time, timestamp(1_767_398_400));
    }
}
