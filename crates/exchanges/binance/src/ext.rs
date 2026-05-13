//! Binance extension field names for request/response fields that do not fit
//! the stable `mkt-types` surface directly.
//!
//! `Extensions` only carries Binance-specific fields that the stable model
//! cannot express yet. Once a field is promoted into `mkt-types`, its Binance
//! extension key must be removed instead of maintaining two copies of the same
//! fact.

// Order identity and execution metadata reported by Binance order APIs.
pub const ORDER_LIST_ID: &str = "binance.order_list_id";
pub const ORIGINAL_CLIENT_ORDER_ID: &str = "binance.original_client_order_id";
pub const CANCEL_CLIENT_ORDER_ID: &str = "binance.cancel_client_order_id";

// Price and execution fields that are only meaningful for specific order shapes.
pub const STOP_PRICE: &str = "binance.stop_price";
pub const ICEBERG_QUANTITY: &str = "binance.iceberg_quantity";
pub const WORKING_TIME: &str = "binance.working_time";
pub const IS_WORKING: &str = "binance.is_working";
pub const SELF_TRADE_PREVENTION_MODE: &str = "binance.self_trade_prevention_mode";

// Placement-specific parameters passed through order requests.
pub const STRATEGY_ID: &str = "binance.strategy_id";
pub const STRATEGY_TYPE: &str = "binance.strategy_type";
pub const TRAILING_DELTA: &str = "binance.trailing_delta";
pub const RECV_WINDOW: &str = "binance.recv_window";

// Trade-level execution metadata returned by fill queries.
pub const QUOTE_QUANTITY: &str = "binance.quote_quantity";
pub const IS_MAKER: &str = "binance.is_maker";
pub const IS_BEST_MATCH: &str = "binance.is_best_match";
pub const PREVENTED_MATCH_ID: &str = "binance.prevented_match_id";
pub const PREVENTED_QUANTITY: &str = "binance.prevented_quantity";
