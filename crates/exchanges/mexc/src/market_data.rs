use std::sync::Arc;

use async_trait::async_trait;
use mkt_core::{MarketData, Result};
use mkt_types::{Kline, KlineRequest, LastPrice, MarketInfo, OrderBook, Symbol, Trade};

use crate::{
    convert, error,
    rest::{query_pair, MexcRestClient},
    MexcInner,
};

const EXCHANGE_INFO_OPERATION: &str = "spot.exchange_info";
const TICKER_PRICE_OPERATION: &str = "spot.ticker_price";
const ORDER_BOOK_OPERATION: &str = "spot.depth";
const RECENT_TRADES_OPERATION: &str = "spot.trades";
const KLINES_OPERATION: &str = "spot.klines";

pub(crate) struct MexcMarketData {
    rest: MexcRestClient,
}

impl MexcMarketData {
    pub(crate) fn new(inner: Arc<MexcInner>) -> Self {
        Self {
            rest: MexcRestClient::new(inner),
        }
    }

    async fn exchange_info(&self, symbol: Option<&Symbol>) -> Result<Vec<MarketInfo>> {
        let mut query = Vec::new();
        if let Some(symbol) = symbol {
            query.push(query_pair(
                "symbol",
                convert::require_spot_symbol(symbol, EXCHANGE_INFO_OPERATION)?,
            ));
        }
        let response: convert::ExchangeInfoResponse = self
            .rest
            .get_public(EXCHANGE_INFO_OPERATION, "api/v3/exchangeInfo", query)
            .await?;

        convert::markets_from_exchange_info_response(response, EXCHANGE_INFO_OPERATION)
    }

    async fn last_prices_for_symbol(&self, symbol: &Symbol) -> Result<Vec<LastPrice>> {
        let response: convert::TickerPriceResponse = self
            .rest
            .get_public(
                TICKER_PRICE_OPERATION,
                "api/v3/ticker/price",
                vec![query_pair(
                    "symbol",
                    convert::require_spot_symbol(symbol, TICKER_PRICE_OPERATION)?,
                )],
            )
            .await?;

        convert::last_prices_from_response(response, TICKER_PRICE_OPERATION)
    }

    async fn all_last_prices(&self) -> Result<Vec<LastPrice>> {
        let response: convert::TickerPriceResponse = self
            .rest
            .get_public(TICKER_PRICE_OPERATION, "api/v3/ticker/price", Vec::new())
            .await?;

        convert::last_prices_from_response(response, TICKER_PRICE_OPERATION)
    }

    #[cfg(test)]
    fn last_price_symbol_queries(symbols: &[Symbol]) -> Result<Vec<Vec<(&'static str, String)>>> {
        symbols
            .iter()
            .map(|symbol| {
                Ok(vec![query_pair(
                    "symbol",
                    convert::require_spot_symbol(symbol, TICKER_PRICE_OPERATION)?,
                )])
            })
            .collect()
    }
}

#[async_trait]
impl MarketData for MexcMarketData {
    async fn markets(&self) -> Result<Vec<MarketInfo>> {
        self.exchange_info(None).await
    }

    async fn market(&self, symbol: &Symbol) -> Result<Option<MarketInfo>> {
        let mut markets = self.exchange_info(Some(symbol)).await?;
        match markets.len() {
            0 => Ok(None),
            1 => Ok(markets.pop()),
            _ => Err(error::invalid_field(
                EXCHANGE_INFO_OPERATION,
                "symbol",
                "expected at most one market in symbol-scoped exchangeInfo response",
            )),
        }
    }

    async fn last_price(&self, symbol: &Symbol) -> Result<LastPrice> {
        let mut prices = self.last_prices(Some(std::slice::from_ref(symbol))).await?;
        match prices.len() {
            1 => Ok(prices.pop().ok_or_else(|| {
                error::invalid_field(
                    TICKER_PRICE_OPERATION,
                    "symbol",
                    "missing ticker price for requested symbol",
                )
            })?),
            0 => Err(error::invalid_field(
                TICKER_PRICE_OPERATION,
                "symbol",
                "missing ticker price for requested symbol",
            )),
            _ => Err(error::invalid_field(
                TICKER_PRICE_OPERATION,
                "symbol",
                "expected a single ticker price for requested symbol",
            )),
        }
    }

    async fn last_prices(&self, symbols: Option<&[Symbol]>) -> Result<Vec<LastPrice>> {
        let Some(symbols) = symbols else {
            return self.all_last_prices().await;
        };
        if symbols.is_empty() {
            return Ok(Vec::new());
        }

        let mut prices = Vec::new();
        for symbol in symbols {
            prices.append(&mut self.last_prices_for_symbol(symbol).await?);
        }
        Ok(prices)
    }

    async fn order_book(&self, symbol: &Symbol, depth: Option<u16>) -> Result<OrderBook> {
        let mut query = vec![query_pair(
            "symbol",
            convert::require_spot_symbol(symbol, ORDER_BOOK_OPERATION)?,
        )];
        if let Some(depth) = depth {
            query.push(query_pair("limit", depth));
        }
        let response: convert::OrderBookResponse = self
            .rest
            .get_public(ORDER_BOOK_OPERATION, "api/v3/depth", query)
            .await?;

        convert::order_book_from_response(symbol, response, ORDER_BOOK_OPERATION)
    }

    async fn recent_trades(&self, symbol: &Symbol, limit: Option<u32>) -> Result<Vec<Trade>> {
        let mut query = vec![query_pair(
            "symbol",
            convert::require_spot_symbol(symbol, RECENT_TRADES_OPERATION)?,
        )];
        if let Some(limit) = limit {
            query.push(query_pair("limit", limit));
        }
        let response: Vec<convert::TradeResponse> = self
            .rest
            .get_public(RECENT_TRADES_OPERATION, "api/v3/trades", query)
            .await?;

        convert::trades_from_response(symbol, response, RECENT_TRADES_OPERATION)
    }

    async fn klines(&self, request: KlineRequest) -> Result<Vec<Kline>> {
        let mut query = vec![
            query_pair(
                "symbol",
                convert::require_spot_symbol(&request.symbol, KLINES_OPERATION)?,
            ),
            query_pair(
                "interval",
                convert::mexc_interval(request.interval, KLINES_OPERATION)?,
            ),
        ];
        if let Some(start) = request.start {
            query.push(query_pair(
                "startTime",
                convert::unix_timestamp_millis(start, KLINES_OPERATION, "startTime")?,
            ));
        }
        if let Some(end) = request.end {
            query.push(query_pair(
                "endTime",
                convert::unix_timestamp_millis(end, KLINES_OPERATION, "endTime")?,
            ));
        }
        if let Some(limit) = request.limit {
            query.push(query_pair("limit", limit));
        }

        let response: Vec<Vec<serde_json::Value>> = self
            .rest
            .get_public(KLINES_OPERATION, "api/v3/klines", query)
            .await?;

        convert::klines_from_rows(
            &request.symbol,
            request.interval,
            response,
            KLINES_OPERATION,
        )
    }
}

#[cfg(test)]
mod tests {
    use mkt_types::Symbol;

    use super::MexcMarketData;

    #[test]
    fn multi_symbol_last_prices_plan_single_symbol_requests() {
        let queries = MexcMarketData::last_price_symbol_queries(&[
            Symbol::spot("BTCUSDT"),
            Symbol::spot("ETHUSDT"),
        ])
        .expect("spot symbols should plan ticker requests");

        assert_eq!(
            queries,
            vec![
                vec![("symbol", String::from("BTCUSDT"))],
                vec![("symbol", String::from("ETHUSDT"))],
            ]
        );
    }
}
