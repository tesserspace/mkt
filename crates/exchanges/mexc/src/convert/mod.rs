//! RATIONALE: This boundary module keeps MEXC REST/stream conversion helpers
//! out of `market_data.rs` so the adapter stays under the repo size guidance.
//! Module-level entry points are intentional here; shared parsing helpers live
//! in `internal.rs`, while protocol-specific DTO conversions are split between
//! `market_data.rs` and `stream/`.

mod internal;
mod market_data;
mod order;
mod spot;
pub(crate) mod stream;

pub(crate) use internal::{mexc_interval, require_spot_symbol, unix_timestamp_millis};
pub(crate) use market_data::{
    klines_from_rows, last_prices_from_response, markets_from_exchange_info_response,
    order_book_from_response, trades_from_response, ExchangeInfoResponse, OrderBookResponse,
    TickerPriceResponse, TradeResponse,
};
pub(crate) use order::{
    order_from_snapshot, DeleteOrderResponse, GetOrderResponse, NewOrderResponse, OpenOrderResponse,
};
pub(crate) use spot::{
    balance_from_account_balance, build_new_order_query, fill_from_trade, lookup_order_key,
    parse_exchange_order_id, AccountResponse, MyTradeResponse,
};
