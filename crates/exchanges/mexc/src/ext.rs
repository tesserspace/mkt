//! MEXC extension field names for request/response fields that do not fit the
//! stable `mkt-types` surface directly.

pub const STOP_PRICE: &str = "mexc.stop_price";
pub const ICEBERG_QUANTITY: &str = "mexc.iceberg_quantity";
pub const IS_WORKING: &str = "mexc.is_working";
pub const IS_MAKER: &str = "mexc.is_maker";
pub const IS_BEST_MATCH: &str = "mexc.is_best_match";
pub const RECV_WINDOW: &str = "mexc.recv_window";
pub const BALANCE_TIMESTAMP_SOURCE: &str = "mexc.balance_timestamp_source";
