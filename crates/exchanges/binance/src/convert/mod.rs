//! RATIONALE: This module is the Binance Spot boundary adapter. Its free
//! functions are intentionally kept as module-level conversion entry points so
//! REST and stream adapters can translate SDK payloads and params without
//! introducing a stateless namespace type. Private helper functions live in
//! `internal.rs` so this boundary file only exposes crate-internal adapter
//! entry points.

mod internal;
mod market_data;
mod order;
mod spot;
pub(crate) mod stream;

pub(crate) use market_data::{
    klines_from_rows, last_prices_from_response, market_info_from_exchange_symbol,
    order_book_from_depth, trades_from_recent_response,
};
pub(crate) use order::order_from_snapshot;
pub(crate) use spot::{
    balance_from_account_balance, build_klines_params, build_new_order_params, fill_from_trade,
    lookup_order_key, parse_exchange_order_id, require_spot_symbol,
};
