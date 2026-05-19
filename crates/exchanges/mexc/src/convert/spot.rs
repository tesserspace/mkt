use mkt_core::Result;
use mkt_types::{
    Balance, Extensions, Fill, OrderId, OrderKey, OrderQuantity, OrderSide, SpotOrderRequest,
    Symbol,
};
use serde::Deserialize;
use time::OffsetDateTime;

use super::{internal, order};
use crate::{error, ext, rest::query_pair};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AccountResponse {
    pub(crate) update_time: Option<i64>,
    #[serde(default)]
    pub(crate) balances: Vec<AccountBalanceResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AccountBalanceResponse {
    pub(crate) asset: Option<String>,
    pub(crate) free: Option<String>,
    pub(crate) locked: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MyTradeResponse {
    #[serde(default)]
    pub(crate) id: Option<serde_json::Value>,
    #[serde(default)]
    pub(crate) order_id: Option<serde_json::Value>,
    pub(crate) price: Option<String>,
    pub(crate) qty: Option<String>,
    pub(crate) quote_qty: Option<String>,
    pub(crate) commission: Option<String>,
    pub(crate) commission_asset: Option<String>,
    pub(crate) time: Option<i64>,
    pub(crate) is_buyer: Option<bool>,
    pub(crate) is_maker: Option<bool>,
    pub(crate) is_best_match: Option<bool>,
}

pub(crate) fn build_new_order_query(
    request: &SpotOrderRequest,
    operation: &'static str,
) -> Result<Vec<(&'static str, String)>> {
    let api_type =
        order::to_api_time_in_force(request.order_type, request.time_in_force, operation)?
            .unwrap_or(order::to_api_order_type(request.order_type, operation)?);
    validate_new_order_request(request, api_type, operation)?;

    let mut query = vec![
        query_pair(
            "symbol",
            internal::require_spot_symbol(&request.symbol, operation)?,
        ),
        query_pair("side", side_to_api(request.side, operation)?),
        query_pair("type", api_type),
    ];
    match request.quantity {
        OrderQuantity::Base(quantity) => query.push(query_pair("quantity", quantity)),
        OrderQuantity::Quote(quantity) => query.push(query_pair("quoteOrderQty", quantity)),
        _ => {
            return Err(error::invalid_field(
                operation,
                "quantity",
                "unsupported MEXC spot quantity mode",
            ))
        }
    }
    if let Some(price) = request.price {
        query.push(query_pair("price", price));
    }
    if let Some(client_order_id) = &request.client_order_id {
        query.push(query_pair("newClientOrderId", client_order_id.0.as_str()));
    }
    if let Some(recv_window) = request
        .extensions
        .i64(ext::RECV_WINDOW)
        .map_err(|err| error::invalid_field(operation, ext::RECV_WINDOW, err.to_string()))?
    {
        query.push(query_pair("recvWindow", recv_window));
    }
    Ok(query)
}

pub(crate) fn lookup_order_key(
    key: &OrderKey,
    operation: &'static str,
) -> Result<(Option<i64>, Option<String>)> {
    match key {
        OrderKey::Exchange(order_id) => {
            Ok((Some(parse_exchange_order_id(&order_id.0, operation)?), None))
        }
        OrderKey::Client(client_order_id) => Ok((None, Some(client_order_id.0.clone()))),
        _ => Err(error::invalid_field(
            operation,
            "key",
            "unsupported MEXC spot order key",
        )),
    }
}

pub(crate) fn parse_exchange_order_id(raw: &str, operation: &'static str) -> Result<i64> {
    raw.parse::<i64>().map_err(|err| {
        error::invalid_field(
            operation,
            "orderId",
            format!(
                "MEXC REST `{operation}` requires a numeric orderId parameter, got `{raw}`: {err}"
            ),
        )
    })
}

pub(crate) fn fill_from_trade(
    symbol: &Symbol,
    trade: MyTradeResponse,
    operation: &'static str,
) -> Result<Fill> {
    let mut extensions = Extensions::new();
    extensions
        .insert_optional_bool(ext::IS_MAKER, trade.is_maker)
        .map_err(|err| error::invalid_field(operation, ext::IS_MAKER, err.to_string()))?;
    extensions
        .insert_optional_bool(ext::IS_BEST_MATCH, trade.is_best_match)
        .map_err(|err| error::invalid_field(operation, ext::IS_BEST_MATCH, err.to_string()))?;

    Fill::builder()
        .id(trade.id.map(internal::value_to_string))
        .order_id(OrderId::new(internal::value_to_string(
            trade
                .order_id
                .ok_or_else(|| error::missing_field(operation, "orderId"))?,
        )))
        .symbol(symbol.clone())
        .side(match trade.is_buyer {
            Some(true) => OrderSide::Buy,
            Some(false) => OrderSide::Sell,
            None => return Err(error::missing_field(operation, "isBuyer")),
        })
        .price(internal::parse_required_decimal(
            trade.price,
            operation,
            "price",
        )?)
        .quantity(internal::parse_required_decimal(
            trade.qty, operation, "qty",
        )?)
        .quote_quantity(internal::parse_optional_decimal(
            trade.quote_qty,
            operation,
            "quoteQty",
        )?)
        .fee(internal::parse_optional_decimal(
            trade.commission,
            operation,
            "commission",
        )?)
        .fee_asset(trade.commission_asset)
        .timestamp(internal::parse_unix_millis_timestamp(
            internal::parse_required_i64(trade.time, operation, "time")?,
            operation,
            "time",
        )?)
        .extensions(extensions)
        .build()
        .map_err(|err| error::invalid_field(operation, "fill", err.to_string()))
}

pub(crate) fn balance_from_account_balance(
    balance: AccountBalanceResponse,
    account_update_time_ms: Option<i64>,
    client_received_at: OffsetDateTime,
    operation: &'static str,
) -> Result<Balance> {
    let available = internal::parse_required_decimal(balance.free, operation, "free")?;
    let locked = internal::parse_required_decimal(balance.locked, operation, "locked")?;
    let mut extensions = Extensions::new();

    // MEXC documents account `updateTime` as nullable. When the venue omits it,
    // this adapter timestamps the received snapshot with client receive time and
    // marks the source so callers can distinguish it from a venue timestamp.
    let (timestamp, source) = match account_update_time_ms {
        Some(update_time) => (
            internal::parse_unix_millis_timestamp(update_time, operation, "updateTime")?,
            "venue_update_time",
        ),
        None => (client_received_at, "client_received_at"),
    };
    extensions
        .insert(
            ext::BALANCE_TIMESTAMP_SOURCE,
            serde_json::Value::String(source.to_owned()),
        )
        .map_err(|err| {
            error::invalid_field(operation, ext::BALANCE_TIMESTAMP_SOURCE, err.to_string())
        })?;

    Balance::builder()
        .asset(
            balance
                .asset
                .ok_or_else(|| error::missing_field(operation, "asset"))?,
        )
        .available(available)
        .locked(locked)
        .total(available + locked)
        .timestamp(timestamp)
        .extensions(extensions)
        .build()
        .map_err(|err| error::invalid_field(operation, "balance", err.to_string()))
}

fn validate_new_order_request(
    request: &SpotOrderRequest,
    api_type: &'static str,
    operation: &'static str,
) -> Result<()> {
    match request.order_type {
        mkt_types::OrderType::Market if request.price.is_some() => Err(error::invalid_field(
            operation,
            "price",
            "market orders must not carry a limit price",
        )),
        mkt_types::OrderType::Limit | mkt_types::OrderType::PostOnly if request.price.is_none() => {
            Err(error::invalid_field(
                operation,
                "price",
                "price is required for MEXC spot limit-style orders",
            ))
        }
        _ if matches!(request.quantity, OrderQuantity::Quote(_))
            && !(request.order_type == mkt_types::OrderType::Market
                && request.side == OrderSide::Buy) =>
        {
            Err(error::invalid_field(
                operation,
                "quantity",
                "quoteOrderQty is only supported for MEXC spot market buys",
            ))
        }
        _ if matches!(api_type, "IMMEDIATE_OR_CANCEL" | "FILL_OR_KILL")
            && request.price.is_none() =>
        {
            Err(error::invalid_field(
                operation,
                "price",
                "IOC/FOK orders require a limit price",
            ))
        }
        _ => Ok(()),
    }
}

fn side_to_api(side: OrderSide, operation: &'static str) -> Result<&'static str> {
    match side {
        OrderSide::Buy => Ok("BUY"),
        OrderSide::Sell => Ok("SELL"),
        _ => Err(error::invalid_field(
            operation,
            "side",
            "unsupported MEXC spot order side",
        )),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use mkt_types::{
        ClientOrderId, OrderKey, OrderQuantity, OrderSide, OrderType, SpotOrderRequest, Symbol,
        TimeInForce,
    };
    use rust_decimal::Decimal;
    use serde_json::json;
    use time::OffsetDateTime;

    use super::{
        balance_from_account_balance, build_new_order_query, fill_from_trade, lookup_order_key,
        parse_exchange_order_id, AccountBalanceResponse, MyTradeResponse,
    };

    const OPERATION: &str = "spot.test";

    #[test]
    fn builds_limit_ioc_order_params_from_unified_request() {
        let request = SpotOrderRequest::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(OrderQuantity::Base(decimal("1.25")))
            .price(Some(decimal("43000.50")))
            .time_in_force(Some(TimeInForce::Ioc))
            .client_order_id(Some(ClientOrderId::new("client-1")))
            .build()
            .expect("complete request should build");

        let params = build_new_order_query(&request, OPERATION)
            .expect("valid request should convert to MEXC params");

        assert!(params.contains(&("symbol", "BTCUSDT".to_owned())));
        assert!(params.contains(&("side", "BUY".to_owned())));
        assert!(params.contains(&("type", "IMMEDIATE_OR_CANCEL".to_owned())));
        assert!(params.contains(&("quantity", "1.25".to_owned())));
        assert!(params.contains(&("price", "43000.50".to_owned())));
        assert!(params.contains(&("newClientOrderId", "client-1".to_owned())));
    }

    #[test]
    fn rejects_unsupported_spot_order_variants() {
        let stop = SpotOrderRequest::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .side(OrderSide::Buy)
            .order_type(OrderType::StopLimit)
            .quantity(OrderQuantity::Base(decimal("1")))
            .price(Some(decimal("43000")))
            .build()
            .expect("request fixture should build");
        let err = build_new_order_query(&stop, OPERATION)
            .expect_err("stop orders should be rejected locally");
        assert!(err
            .to_string()
            .contains("does not officially document stop order placement"));

        let quote_sell = SpotOrderRequest::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .side(OrderSide::Sell)
            .order_type(OrderType::Market)
            .quantity(OrderQuantity::Quote(decimal("100")))
            .build()
            .expect("request fixture should build");
        let err = build_new_order_query(&quote_sell, OPERATION)
            .expect_err("quote sell should be rejected");
        assert!(err.to_string().contains("quoteOrderQty is only supported"));
    }

    #[test]
    fn maps_fills_and_balances_with_timestamp_source() {
        let trade: MyTradeResponse = serde_json::from_value(json!({
            "id": 9001,
            "orderId": 42,
            "price": "2500.25",
            "qty": "0.4",
            "quoteQty": "1000.10",
            "commission": "0.0004",
            "commissionAsset": "ETH",
            "time": 1700000000123i64,
            "isBuyer": false,
            "isMaker": true,
            "isBestMatch": true
        }))
        .expect("trade fixture should deserialize");
        let fill = fill_from_trade(&Symbol::spot("ETHUSDT"), trade, OPERATION)
            .expect("trade fixture should map");
        assert_eq!(fill.order_id.0, "42");
        assert_eq!(fill.side, OrderSide::Sell);
        assert_eq!(fill.price, decimal("2500.25"));
        assert_eq!(fill.quote_quantity, Some(decimal("1000.10")));

        let received_at = OffsetDateTime::from_unix_timestamp(1_700_000_001)
            .expect("timestamp fixture should be valid");
        let balance = balance_from_account_balance(
            AccountBalanceResponse {
                asset: Some("USDT".to_owned()),
                free: Some("10.5".to_owned()),
                locked: Some("1.25".to_owned()),
            },
            None,
            received_at,
            OPERATION,
        )
        .expect("balance fixture should map");
        assert_eq!(balance.total, decimal("11.75"));
        assert_eq!(balance.timestamp, received_at);
        assert_eq!(
            balance
                .extensions
                .string(crate::ext::BALANCE_TIMESTAMP_SOURCE)
                .expect("timestamp source should be textual"),
            Some("client_received_at".to_owned())
        );
    }

    #[test]
    fn non_numeric_exchange_order_ids_fail_only_where_numeric_rest_params_are_required() {
        let err = parse_exchange_order_id("abc-123", OPERATION)
            .expect_err("non-numeric order ids must fail when REST requires numeric orderId");
        assert!(err
            .to_string()
            .contains("requires a numeric orderId parameter"));

        let err = lookup_order_key(
            &OrderKey::Exchange(mkt_types::OrderId::new("not-a-number")),
            OPERATION,
        )
        .expect_err("non-numeric exchange order key must fail for numeric-REST workflows");
        assert!(err
            .to_string()
            .contains("requires a numeric orderId parameter"));
    }

    fn decimal(raw: &str) -> Decimal {
        Decimal::from_str(raw).expect("decimal fixture should be valid")
    }
}
