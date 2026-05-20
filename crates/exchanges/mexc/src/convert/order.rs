//! RATIONALE: This module is the MEXC spot order boundary adapter.
//! It converts the documented REST order payloads into `mkt-types::Order`
//! without inventing missing required fields. Private parsing helpers live in
//! `internal.rs`; this file stays focused on snapshot-to-model conversion.

use serde::Deserialize;

use crate::{error, ext};
use mkt_core::Result;
use mkt_types::{
    ClientOrderId, Extensions, MarketKind, Order, OrderId, OrderStatus, OrderType, Symbol,
    TimeInForce,
};
use time::OffsetDateTime;

#[non_exhaustive]
#[derive(Debug, Default)]
pub(crate) struct MexcOrderSnapshot {
    pub(super) symbol: Option<String>,
    pub(super) order_id: Option<String>,
    pub(super) client_order_id: Option<String>,
    pub(super) price: Option<String>,
    pub(super) orig_qty: Option<String>,
    pub(super) executed_qty: Option<String>,
    pub(super) cummulative_quote_qty: Option<String>,
    pub(super) status: Option<String>,
    pub(super) time_in_force: Option<String>,
    pub(super) order_type: Option<String>,
    pub(super) side: Option<String>,
    pub(super) stop_price: Option<String>,
    pub(super) iceberg_qty: Option<String>,
    pub(super) time: Option<i64>,
    pub(super) update_time: Option<i64>,
    pub(super) is_working: Option<bool>,
}

impl From<NewOrderResponse> for MexcOrderSnapshot {
    fn from(value: NewOrderResponse) -> Self {
        Self {
            symbol: value.symbol,
            order_id: value.order_id.map(super::internal::value_to_string),
            client_order_id: value.client_order_id,
            price: value.price,
            orig_qty: value.orig_qty,
            executed_qty: value.executed_qty,
            cummulative_quote_qty: value.cummulative_quote_qty,
            status: value.status,
            time_in_force: value.time_in_force,
            order_type: value.r#type,
            side: value.side,
            stop_price: value.stop_price,
            iceberg_qty: value.iceberg_qty,
            time: value.transact_time,
            update_time: value.transact_time,
            is_working: value.is_working,
        }
    }
}

impl From<GetOrderResponse> for MexcOrderSnapshot {
    fn from(value: GetOrderResponse) -> Self {
        Self {
            symbol: value.symbol,
            order_id: value.order_id.map(super::internal::value_to_string),
            client_order_id: value.client_order_id,
            price: value.price,
            orig_qty: value.orig_qty,
            executed_qty: value.executed_qty,
            cummulative_quote_qty: value.cummulative_quote_qty,
            status: value.status,
            time_in_force: value.time_in_force,
            order_type: value.r#type,
            side: value.side,
            stop_price: value.stop_price,
            iceberg_qty: value.iceberg_qty,
            time: value.time,
            update_time: value.update_time,
            is_working: value.is_working,
        }
    }
}

impl From<DeleteOrderResponse> for MexcOrderSnapshot {
    fn from(value: DeleteOrderResponse) -> Self {
        Self {
            symbol: value.symbol,
            order_id: value.order_id.map(super::internal::value_to_string),
            client_order_id: value.client_order_id,
            price: value.price,
            orig_qty: value.orig_qty,
            executed_qty: value.executed_qty,
            cummulative_quote_qty: value.cummulative_quote_qty,
            status: value.status,
            time_in_force: value.time_in_force,
            order_type: value.r#type,
            side: value.side,
            stop_price: value.stop_price,
            iceberg_qty: value.iceberg_qty,
            time: value.transact_time,
            update_time: value.transact_time,
            is_working: value.is_working,
        }
    }
}

impl From<OpenOrderResponse> for MexcOrderSnapshot {
    fn from(value: OpenOrderResponse) -> Self {
        Self {
            symbol: value.symbol,
            order_id: value.order_id.map(super::internal::value_to_string),
            client_order_id: value.client_order_id,
            price: value.price,
            orig_qty: value.orig_qty,
            executed_qty: value.executed_qty,
            cummulative_quote_qty: value.cummulative_quote_qty,
            status: value.status,
            time_in_force: value.time_in_force,
            order_type: value.r#type,
            side: value.side,
            stop_price: value.stop_price,
            iceberg_qty: value.iceberg_qty,
            time: value.time,
            update_time: value.update_time,
            is_working: value.is_working,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NewOrderResponse {
    pub(crate) symbol: Option<String>,
    pub(crate) order_id: Option<serde_json::Value>,
    pub(crate) client_order_id: Option<String>,
    pub(crate) price: Option<String>,
    pub(crate) orig_qty: Option<String>,
    pub(crate) executed_qty: Option<String>,
    pub(crate) cummulative_quote_qty: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) time_in_force: Option<String>,
    pub(crate) r#type: Option<String>,
    pub(crate) side: Option<String>,
    pub(crate) stop_price: Option<String>,
    pub(crate) iceberg_qty: Option<String>,
    pub(crate) transact_time: Option<i64>,
    pub(crate) is_working: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GetOrderResponse {
    pub(crate) symbol: Option<String>,
    pub(crate) order_id: Option<serde_json::Value>,
    pub(crate) client_order_id: Option<String>,
    pub(crate) price: Option<String>,
    pub(crate) orig_qty: Option<String>,
    pub(crate) executed_qty: Option<String>,
    pub(crate) cummulative_quote_qty: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) time_in_force: Option<String>,
    pub(crate) r#type: Option<String>,
    pub(crate) side: Option<String>,
    pub(crate) stop_price: Option<String>,
    pub(crate) iceberg_qty: Option<String>,
    pub(crate) time: Option<i64>,
    pub(crate) update_time: Option<i64>,
    pub(crate) is_working: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteOrderResponse {
    pub(crate) symbol: Option<String>,
    pub(crate) order_id: Option<serde_json::Value>,
    pub(crate) client_order_id: Option<String>,
    pub(crate) price: Option<String>,
    pub(crate) orig_qty: Option<String>,
    pub(crate) executed_qty: Option<String>,
    pub(crate) cummulative_quote_qty: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) time_in_force: Option<String>,
    pub(crate) r#type: Option<String>,
    pub(crate) side: Option<String>,
    pub(crate) stop_price: Option<String>,
    pub(crate) iceberg_qty: Option<String>,
    pub(crate) transact_time: Option<i64>,
    pub(crate) is_working: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenOrderResponse {
    pub(crate) symbol: Option<String>,
    pub(crate) order_id: Option<serde_json::Value>,
    pub(crate) client_order_id: Option<String>,
    pub(crate) price: Option<String>,
    pub(crate) orig_qty: Option<String>,
    pub(crate) executed_qty: Option<String>,
    pub(crate) cummulative_quote_qty: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) time_in_force: Option<String>,
    pub(crate) r#type: Option<String>,
    pub(crate) side: Option<String>,
    pub(crate) stop_price: Option<String>,
    pub(crate) iceberg_qty: Option<String>,
    pub(crate) time: Option<i64>,
    pub(crate) update_time: Option<i64>,
    pub(crate) is_working: Option<bool>,
}

pub(crate) fn order_from_snapshot(
    snapshot: MexcOrderSnapshot,
    operation: &'static str,
) -> Result<Order> {
    let raw_status = snapshot
        .status
        .as_deref()
        .ok_or_else(|| error::missing_field(operation, "status"))?;
    let raw_order_type = snapshot
        .order_type
        .as_deref()
        .ok_or_else(|| error::missing_field(operation, "type"))?;
    let side = snapshot
        .side
        .as_deref()
        .ok_or_else(|| error::missing_field(operation, "side"))?;
    let status = order_status_from_raw(raw_status, operation)?;
    let order_type = order_type_from_raw(raw_order_type, operation)?;
    let mut extensions = Extensions::new();
    extensions
        .insert_optional_string(ext::STOP_PRICE, snapshot.stop_price.clone())
        .map_err(|err| error::invalid_field(operation, ext::STOP_PRICE, err.to_string()))?;
    extensions
        .insert_optional_string(ext::ICEBERG_QUANTITY, snapshot.iceberg_qty.clone())
        .map_err(|err| error::invalid_field(operation, ext::ICEBERG_QUANTITY, err.to_string()))?;
    extensions
        .insert_optional_bool(ext::IS_WORKING, snapshot.is_working)
        .map_err(|err| error::invalid_field(operation, ext::IS_WORKING, err.to_string()))?;

    let quantity = parse_required_decimal(snapshot.orig_qty, operation, "origQty")?;
    let filled_quantity = parse_required_decimal(snapshot.executed_qty, operation, "executedQty")?;
    let created_at = parse_unix_millis_timestamp_or_now(
        snapshot.time.or(snapshot.update_time),
        operation,
        "time",
    )?;
    let updated_at =
        parse_optional_unix_millis_timestamp(snapshot.update_time, operation, "updateTime")?;
    let order_id = snapshot
        .order_id
        .ok_or_else(|| error::missing_field(operation, "orderId"))?;

    let mut builder = Order::builder()
        .id(OrderId::new(order_id))
        .symbol(Symbol::spot(
            snapshot
                .symbol
                .ok_or_else(|| error::missing_field(operation, "symbol"))?,
        ))
        .market_kind(MarketKind::Spot)
        .side(order_side_from_raw(side, operation)?)
        .order_type(order_type)
        .status(status)
        .quantity(quantity)
        .filled_quantity(filled_quantity)
        .created_at(created_at)
        .extensions(extensions);
    if let Some(client_order_id) = snapshot.client_order_id {
        builder = builder.client_order_id(Some(ClientOrderId::new(client_order_id)));
    }
    if let Some(time_in_force) = snapshot.time_in_force {
        builder = builder.time_in_force(Some(time_in_force_from_raw(
            time_in_force.as_str(),
            operation,
        )?));
    } else if let Some(inferred) = infer_time_in_force(raw_order_type) {
        builder = builder.time_in_force(Some(inferred));
    }
    if let Some(price) = snapshot.price {
        builder = builder.price(Some(parse_decimal(price, operation, "price")?));
    }
    if let Some(cumulative) = snapshot.cummulative_quote_qty {
        builder = builder.cumulative_quote_quantity(Some(parse_decimal(
            cumulative,
            operation,
            "cummulativeQuoteQty",
        )?));
    }
    builder
        .updated_at(updated_at)
        .build()
        .map_err(|err| error::invalid_field(operation, "order", err.to_string()))
}

pub(crate) fn order_status_from_raw(raw: &str, operation: &'static str) -> Result<OrderStatus> {
    match raw {
        "NEW" => Ok(OrderStatus::New),
        "PARTIALLY_FILLED" => Ok(OrderStatus::PartiallyFilled),
        "FILLED" => Ok(OrderStatus::Filled),
        "CANCELED" | "CANCELLED" => Ok(OrderStatus::Canceled),
        "PARTIALLY_CANCELED" | "PARTIALLY_CANCELLED" => Ok(OrderStatus::Canceled),
        "REJECTED" => Ok(OrderStatus::Rejected),
        "EXPIRED" => Ok(OrderStatus::Expired),
        other => Err(error::invalid_field(
            operation,
            "status",
            format!("unsupported MEXC spot order status `{other}`"),
        )),
    }
}

pub(crate) fn order_type_from_raw(raw: &str, operation: &'static str) -> Result<OrderType> {
    match raw {
        "LIMIT" => Ok(OrderType::Limit),
        "MARKET" => Ok(OrderType::Market),
        "LIMIT_MAKER" => Ok(OrderType::PostOnly),
        "IMMEDIATE_OR_CANCEL" | "IOC" | "FILL_OR_KILL" | "FOK" => Ok(OrderType::Limit),
        "STOP_LOSS" | "STOP_LOSS_LIMIT" | "TAKE_PROFIT" | "TAKE_PROFIT_LIMIT" => Err(
            error::invalid_field(operation, "type", "unsupported MEXC spot stop order type"),
        ),
        other => Err(error::invalid_field(
            operation,
            "type",
            format!("unsupported MEXC spot order type `{other}`"),
        )),
    }
}

pub(crate) fn to_api_order_type(
    order_type: OrderType,
    operation: &'static str,
) -> Result<&'static str> {
    match order_type {
        OrderType::Market => Ok("MARKET"),
        OrderType::Limit => Ok("LIMIT"),
        OrderType::PostOnly => Ok("LIMIT_MAKER"),
        OrderType::StopMarket | OrderType::StopLimit => Err(error::invalid_field(
            operation,
            "type",
            "MEXC spot REST does not officially document stop order placement on /api/v3/order",
        )),
        _ => Err(error::invalid_field(
            operation,
            "type",
            "unsupported spot order type",
        )),
    }
}

pub(crate) fn to_api_time_in_force(
    order_type: OrderType,
    time_in_force: Option<TimeInForce>,
    operation: &'static str,
) -> Result<Option<&'static str>> {
    match order_type {
        OrderType::Limit => match time_in_force.unwrap_or(TimeInForce::Gtc) {
            TimeInForce::Gtc => Ok(Some("LIMIT")),
            TimeInForce::Ioc => Ok(Some("IMMEDIATE_OR_CANCEL")),
            TimeInForce::Fok => Ok(Some("FILL_OR_KILL")),
            TimeInForce::Gtx => Err(error::invalid_field(
                operation,
                "time_in_force",
                "MEXC spot does not support GTX",
            )),
            _ => Err(error::invalid_field(
                operation,
                "time_in_force",
                "unsupported MEXC spot time in force",
            )),
        },
        OrderType::Market | OrderType::PostOnly => Ok(None),
        OrderType::StopMarket | OrderType::StopLimit => Err(error::invalid_field(
            operation,
            "time_in_force",
            "MEXC spot REST does not officially document stop order placement on /api/v3/order",
        )),
        _ => Err(error::invalid_field(
            operation,
            "time_in_force",
            "unsupported spot order type",
        )),
    }
}

fn order_side_from_raw(raw: &str, operation: &'static str) -> Result<mkt_types::OrderSide> {
    match raw {
        "BUY" => Ok(mkt_types::OrderSide::Buy),
        "SELL" => Ok(mkt_types::OrderSide::Sell),
        other => Err(error::invalid_field(
            operation,
            "side",
            format!("unsupported MEXC spot order side `{other}`"),
        )),
    }
}

fn parse_decimal(
    raw: String,
    operation: &'static str,
    field: &'static str,
) -> Result<mkt_types::Decimal> {
    super::internal::parse_decimal(raw, operation, field)
}

fn parse_required_decimal(
    raw: Option<String>,
    operation: &'static str,
    field: &'static str,
) -> Result<mkt_types::Decimal> {
    super::internal::parse_required_decimal(raw, operation, field)
}

fn parse_unix_millis_timestamp_or_now(
    raw: Option<i64>,
    operation: &'static str,
    field: &'static str,
) -> Result<time::OffsetDateTime> {
    raw.map(|value| super::internal::parse_unix_millis_timestamp(value, operation, field))
        .transpose()
        .map(|timestamp| timestamp.unwrap_or_else(OffsetDateTime::now_utc))
}

fn parse_optional_unix_millis_timestamp(
    raw: Option<i64>,
    operation: &'static str,
    field: &'static str,
) -> Result<Option<time::OffsetDateTime>> {
    raw.map(|value| super::internal::parse_unix_millis_timestamp(value, operation, field))
        .transpose()
}

fn time_in_force_from_raw(raw: &str, operation: &'static str) -> Result<TimeInForce> {
    match raw {
        "GTC" => Ok(TimeInForce::Gtc),
        "IOC" => Ok(TimeInForce::Ioc),
        "FOK" => Ok(TimeInForce::Fok),
        other => Err(error::invalid_field(
            operation,
            "timeInForce",
            format!("unsupported MEXC spot time in force `{other}`"),
        )),
    }
}

fn infer_time_in_force(raw_order_type: &str) -> Option<TimeInForce> {
    match raw_order_type {
        "LIMIT" => Some(TimeInForce::Gtc),
        "IMMEDIATE_OR_CANCEL" | "IOC" => Some(TimeInForce::Ioc),
        "FILL_OR_KILL" | "FOK" => Some(TimeInForce::Fok),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        order_from_snapshot, order_status_from_raw, DeleteOrderResponse, GetOrderResponse,
        NewOrderResponse,
    };
    use mkt_types::OrderStatus;
    use serde_json::json;

    const OPERATION: &str = "spot.order.test";

    #[test]
    fn string_order_id_snapshots_deserialize_and_map_without_loss() {
        let response: GetOrderResponse = serde_json::from_value(json!({
            "symbol": "BTCUSDT",
            "orderId": "abc-123",
            "clientOrderId": "client-1",
            "price": "43000.50",
            "origQty": "1.25",
            "executedQty": "0.25",
            "cummulativeQuoteQty": "10750.125",
            "status": "PARTIALLY_FILLED",
            "timeInForce": "GTC",
            "type": "LIMIT",
            "side": "BUY",
            "time": 1700000000000i64,
            "updateTime": 1700000005000i64,
            "isWorking": true
        }))
        .expect("string orderId fixture should deserialize");

        let order = order_from_snapshot(response.into(), OPERATION)
            .expect("string orderId snapshot should map to Order");
        assert_eq!(order.id.0, "abc-123");
    }

    #[test]
    fn numeric_order_id_snapshots_remain_compatible() {
        let response: NewOrderResponse = serde_json::from_value(json!({
            "symbol": "BTCUSDT",
            "orderId": 42,
            "clientOrderId": "client-1",
            "price": "43000.50",
            "origQty": "1.25",
            "executedQty": "0",
            "cummulativeQuoteQty": "0",
            "status": "NEW",
            "timeInForce": "GTC",
            "type": "LIMIT",
            "side": "BUY",
            "transactTime": 1700000000000i64,
            "isWorking": true
        }))
        .expect("numeric orderId fixture should deserialize");

        let order = order_from_snapshot(response.into(), OPERATION)
            .expect("numeric orderId snapshot should map to Order");
        assert_eq!(order.id.0, "42");
    }

    #[test]
    fn delete_order_response_accepts_string_order_id() {
        let response: DeleteOrderResponse = serde_json::from_value(json!({
            "symbol": "BTCUSDT",
            "orderId": "9007199254740993",
            "clientOrderId": "client-1",
            "price": "43000.50",
            "origQty": "1.25",
            "executedQty": "1.25",
            "cummulativeQuoteQty": "53750.625",
            "status": "CANCELED",
            "timeInForce": "GTC",
            "type": "LIMIT",
            "side": "BUY",
            "transactTime": 1700000000000i64,
            "isWorking": false
        }))
        .expect("string orderId delete response should deserialize");

        let order = order_from_snapshot(response.into(), OPERATION)
            .expect("delete snapshot should map to Order");
        assert_eq!(order.id.0, "9007199254740993");
    }

    #[test]
    fn cancel_response_without_time_uses_client_receive_time() {
        let response: DeleteOrderResponse = serde_json::from_value(json!({
            "symbol": "BTCUSDT",
            "orderId": "C02__123",
            "clientOrderId": "client-1",
            "price": "43000.50",
            "origQty": "1.25",
            "executedQty": "0",
            "cummulativeQuoteQty": "0",
            "status": "CANCELED",
            "timeInForce": "GTC",
            "type": "LIMIT",
            "side": "SELL",
            "isWorking": false
        }))
        .expect("MEXC cancel fixture should deserialize without time");

        let order = order_from_snapshot(response.into(), OPERATION)
            .expect("cancel snapshot without time should map to Order");
        assert_eq!(order.id.0, "C02__123");
        assert_eq!(order.status, OrderStatus::Canceled);
    }

    #[test]
    fn partially_canceled_status_maps_to_terminal_canceled() {
        assert_eq!(
            order_status_from_raw("PARTIALLY_CANCELED", OPERATION)
                .expect("documented MEXC spelling should map"),
            OrderStatus::Canceled
        );
        assert_eq!(
            order_status_from_raw("PARTIALLY_CANCELLED", OPERATION)
                .expect("alternate spelling should map"),
            OrderStatus::Canceled
        );
    }
}
