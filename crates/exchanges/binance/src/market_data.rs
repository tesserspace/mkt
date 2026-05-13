use std::sync::Arc;

use async_trait::async_trait;
use binance_sdk::spot::rest_api::{
    DepthParams, ExchangeInfoParams, GetTradesParams, TickerPriceParams,
};
use mkt_core::{MarketData, Result};
use mkt_types::{Kline, KlineRequest, LastPrice, MarketInfo, OrderBook, Symbol, Trade};

use crate::{convert, error, BinanceInner};

const EXCHANGE_INFO_OPERATION: &str = "spot.exchange_info";
const TICKER_PRICE_OPERATION: &str = "spot.ticker_price";
const ORDER_BOOK_OPERATION: &str = "spot.depth";
const RECENT_TRADES_OPERATION: &str = "spot.get_trades";
const KLINES_OPERATION: &str = "spot.klines";

pub(crate) struct BinanceMarketData {
    inner: Arc<BinanceInner>,
}

impl BinanceMarketData {
    pub(crate) fn new(inner: Arc<BinanceInner>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl MarketData for BinanceMarketData {
    async fn markets(&self) -> Result<Vec<MarketInfo>> {
        let params = ExchangeInfoParams::builder()
            .show_permission_sets(false)
            .build()
            .map_err(|err| error::adapter_error(EXCHANGE_INFO_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .exchange_info(params)
            .await
            .map_err(|err| error::map_request_error(EXCHANGE_INFO_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(EXCHANGE_INFO_OPERATION, err))?;

        data.symbols
            .unwrap_or_default()
            .into_iter()
            .map(|symbol| {
                convert::market_info_from_exchange_symbol(symbol, EXCHANGE_INFO_OPERATION)
            })
            .collect()
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
                "expected a single-symbol ticker price response",
            )),
        }
    }

    async fn last_prices(&self, symbols: Option<&[Symbol]>) -> Result<Vec<LastPrice>> {
        let Some(symbols) = symbols else {
            let response = self
                .inner
                .spot_rest
                .ticker_price(TickerPriceParams::default())
                .await
                .map_err(|err| error::map_request_error(TICKER_PRICE_OPERATION, err))?;
            let data = response
                .data()
                .await
                .map_err(|err| error::map_connector_error(TICKER_PRICE_OPERATION, err))?;

            return convert::last_prices_from_response(data, TICKER_PRICE_OPERATION);
        };

        if symbols.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder = TickerPriceParams::builder();
        if symbols.len() == 1 {
            builder = builder.symbol(convert::require_spot_symbol(
                &symbols[0],
                TICKER_PRICE_OPERATION,
            )?);
        } else {
            let symbol_names = symbols
                .iter()
                .map(|symbol| convert::require_spot_symbol(symbol, TICKER_PRICE_OPERATION))
                .collect::<Result<Vec<_>>>()?;
            builder = builder.symbols(symbol_names);
        }
        let params = builder
            .build()
            .map_err(|err| error::adapter_error(TICKER_PRICE_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .ticker_price(params)
            .await
            .map_err(|err| error::map_request_error(TICKER_PRICE_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(TICKER_PRICE_OPERATION, err))?;

        convert::last_prices_from_response(data, TICKER_PRICE_OPERATION)
    }

    async fn order_book(&self, symbol: &Symbol, depth: Option<u32>) -> Result<OrderBook> {
        let mut builder =
            DepthParams::builder(convert::require_spot_symbol(symbol, ORDER_BOOK_OPERATION)?);
        if let Some(depth) = depth {
            builder = builder.limit(i32::try_from(depth).map_err(|_| {
                error::invalid_field(
                    ORDER_BOOK_OPERATION,
                    "depth",
                    "depth is out of i32 range for Binance spot",
                )
            })?);
        }
        let params = builder
            .build()
            .map_err(|err| error::adapter_error(ORDER_BOOK_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .depth(params)
            .await
            .map_err(|err| error::map_request_error(ORDER_BOOK_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(ORDER_BOOK_OPERATION, err))?;

        convert::order_book_from_depth(symbol, data, ORDER_BOOK_OPERATION)
    }

    async fn recent_trades(&self, symbol: &Symbol, limit: Option<u32>) -> Result<Vec<Trade>> {
        let mut builder = GetTradesParams::builder(convert::require_spot_symbol(
            symbol,
            RECENT_TRADES_OPERATION,
        )?);
        if let Some(limit) = limit {
            builder = builder.limit(i32::try_from(limit).map_err(|_| {
                error::invalid_field(
                    RECENT_TRADES_OPERATION,
                    "limit",
                    "limit is out of i32 range for Binance spot",
                )
            })?);
        }
        let params = builder
            .build()
            .map_err(|err| error::adapter_error(RECENT_TRADES_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .get_trades(params)
            .await
            .map_err(|err| error::map_request_error(RECENT_TRADES_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(RECENT_TRADES_OPERATION, err))?;

        convert::trades_from_recent_response(symbol, data, RECENT_TRADES_OPERATION)
    }

    async fn klines(&self, request: KlineRequest) -> Result<Vec<Kline>> {
        let params = convert::build_klines_params(&request, KLINES_OPERATION)?;
        let symbol = request.symbol;
        let request_interval = request.interval;
        let response = self
            .inner
            .spot_rest
            .klines(params)
            .await
            .map_err(|err| error::map_request_error(KLINES_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(KLINES_OPERATION, err))?;

        convert::klines_from_rows(&symbol, request_interval, data, KLINES_OPERATION)
    }
}
