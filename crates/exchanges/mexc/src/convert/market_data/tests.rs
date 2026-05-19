use super::{
    kline_from_row, last_prices_from_response, market_info_from_response, market_status_from_raw,
    order_book_from_response, ExchangeInfoSymbolResponse, OrderBookResponse, TickerPriceResponse,
};
use mkt_types::{DerivativeKind, KlineInterval, MarketStatus, SettlementMode, Symbol};
use rust_decimal::Decimal;
use serde_json::json;
use std::str::FromStr;
use time::{Duration, OffsetDateTime};

#[test]
fn exchange_info_precision_mapping_uses_true_precision_fields_only() {
    let raw = r#"{
        "symbol":"BTCUSDT",
        "status":"1",
        "baseAsset":"BTC",
        "quoteAsset":"USDT",
        "baseAssetPrecision":6,
        "quotePrecision":"2",
        "quoteAssetPrecision":8,
        "baseSizePrecision":"0.0001",
        "quoteAmountPrecision":"5"
    }"#;
    let parsed: ExchangeInfoSymbolResponse =
        serde_json::from_str(raw).expect("fixture should deserialize");

    let market = market_info_from_response(parsed, "test")
        .expect("precision fixture should map to MarketInfo");
    assert_eq!(market.base_asset_precision, Some(6));
    assert_eq!(market.quote_asset_precision, Some(8));
    assert_eq!(market.quote_precision, Some(2));
}

#[test]
fn exchange_info_status_maps_documented_mexc_values() {
    assert_eq!(
        market_status_from_raw("1", "test").expect("numeric trading status should map"),
        MarketStatus::Trading
    );
    assert_eq!(
        market_status_from_raw("ENABLED", "test").expect("text trading status should map"),
        MarketStatus::Trading
    );
    assert_eq!(
        market_status_from_raw("2", "test").expect("numeric halted status should map"),
        MarketStatus::Halted
    );
    assert_eq!(
        market_status_from_raw("OFFLINE", "test").expect("text halted status should map"),
        MarketStatus::Halted
    );
    assert_eq!(
        market_status_from_raw("3", "test").expect("numeric offline status should map"),
        MarketStatus::Halted
    );
    assert_eq!(
        market_status_from_raw("PAUSE", "test").expect("text paused status should map"),
        MarketStatus::Halted
    );
}

#[test]
fn rejects_non_spot_symbols_for_market_data() {
    let symbol = Symbol::derivative(DerivativeKind::perpetual(SettlementMode::Linear), "BTCUSDT");
    let err = super::super::internal::require_spot_symbol(&symbol, "test")
        .expect_err("derivative symbols must fail");
    assert!(err.to_string().contains("only accepts spot symbols"));
}

#[test]
fn supports_documented_mexc_intervals() {
    assert_eq!(
        super::super::internal::mexc_interval(KlineInterval::Minute(1), "test")
            .expect("1m is supported"),
        "1m"
    );
    assert_eq!(
        super::super::internal::mexc_interval(KlineInterval::Hour(1), "test")
            .expect("1h must map to MEXC 60m"),
        "60m"
    );
    assert_eq!(
        super::super::internal::mexc_interval(KlineInterval::Week(1), "test")
            .expect("1w is supported"),
        "1W"
    );
    assert_eq!(
        super::super::internal::mexc_interval(KlineInterval::Month(1), "test")
            .expect("1M is supported"),
        "1M"
    );
    let err = super::super::internal::mexc_interval(KlineInterval::Hour(2), "test")
        .expect_err("2h should not be claimed without explicit support");
    assert!(err
        .to_string()
        .contains("unsupported MEXC spot kline interval"));
}

#[test]
fn ticker_conversion_reports_missing_and_invalid_fields() {
    let missing = serde_json::from_value::<TickerPriceResponse>(json!({
        "symbol": "BTCUSDT"
    }))
    .expect("optional DTO fields should deserialize");
    let err = last_prices_from_response(missing, "test")
        .expect_err("missing price should be field-specific");
    assert!(err
        .to_string()
        .contains("missing required MEXC field `price`"));

    let invalid = serde_json::from_value::<TickerPriceResponse>(json!({
        "symbol": "BTCUSDT",
        "price": "not-a-decimal"
    }))
    .expect("optional DTO fields should deserialize");
    let err = last_prices_from_response(invalid, "test")
        .expect_err("invalid price should be field-specific");
    assert!(err.to_string().contains("invalid `price`"));
}

#[test]
fn order_book_preserves_last_update_id() {
    let response: OrderBookResponse = serde_json::from_value(json!({
        "lastUpdateId": 12345,
        "bids": [["1.0", "2.0"]],
        "asks": [["1.1", "3.0"]]
    }))
    .expect("order book fixture should deserialize");

    let book = order_book_from_response(&Symbol::spot("BTCUSDT"), response, "test")
        .expect("order book should convert");
    assert_eq!(book.last_update_id.as_deref(), Some("12345"));
}

#[test]
fn kline_closed_uses_close_time_and_rejects_negative_timestamps() {
    let past_close_ms = unix_millis(OffsetDateTime::now_utc() - Duration::minutes(1));
    let future_close_ms = unix_millis(OffsetDateTime::now_utc() + Duration::minutes(1));

    let past = kline_from_row(
        &Symbol::spot("BTCUSDT"),
        KlineInterval::Minute(1),
        kline_row(0, past_close_ms),
        "test",
    )
    .expect("past kline should convert");
    assert!(past.closed);

    let future = kline_from_row(
        &Symbol::spot("BTCUSDT"),
        KlineInterval::Minute(1),
        kline_row(0, future_close_ms),
        "test",
    )
    .expect("future kline should convert");
    assert!(!future.closed);

    let err = kline_from_row(
        &Symbol::spot("BTCUSDT"),
        KlineInterval::Minute(1),
        kline_row(-1, past_close_ms),
        "test",
    )
    .expect_err("negative open timestamp should fail");
    assert!(err
        .to_string()
        .contains("invalid Unix millisecond timestamp"));
}

fn kline_row(open_time: i64, close_time: i64) -> Vec<serde_json::Value> {
    vec![
        json!(open_time),
        json!("1.0"),
        json!("2.0"),
        json!("0.5"),
        json!("1.5"),
        json!("10.0"),
        json!(close_time),
        json!("15.0"),
    ]
}

fn unix_millis(timestamp: OffsetDateTime) -> i64 {
    i64::try_from(timestamp.unix_timestamp_nanos() / 1_000_000)
        .expect("test timestamp must fit i64 milliseconds")
}

#[test]
fn decimal_fixtures_are_exact() {
    assert_eq!(
        Decimal::from_str("1.0").expect("decimal fixture should parse"),
        Decimal::ONE
    );
}
