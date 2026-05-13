//! RATIONALE: This module is the Binance Spot order response boundary adapter.
//! It converts the SDK's per-endpoint order payload shapes into the stable
//! `mkt-types::Order` model while keeping only Binance-specific metadata in
//! `Extensions`. Private helper functions live in `internal.rs`; this file
//! keeps only the crate-internal order conversion entry point.

use binance_sdk::spot::rest_api::{
    AllOrdersResponseInner, DeleteOrderResponse, GetOrderResponse, NewOrderResponse,
};
use mkt_core::Result;
use mkt_types::{ClientOrderId, Extensions, MarketKind, Order, OrderId, Symbol};

use super::internal;

#[non_exhaustive]
#[derive(Debug, Default)]
pub(crate) struct BinanceOrderSnapshot {
    pub(super) symbol: Option<String>,
    pub(super) order_id: Option<i64>,
    pub(super) order_list_id: Option<i64>,
    pub(super) client_order_id: Option<String>,
    pub(super) original_client_order_id: Option<String>,
    pub(super) transaction_time: Option<i64>,
    pub(super) price: Option<String>,
    pub(super) original_quantity: Option<String>,
    pub(super) executed_quantity: Option<String>,
    pub(super) original_quote_quantity: Option<String>,
    pub(super) cumulative_quote_quantity: Option<String>,
    pub(super) status: Option<String>,
    pub(super) time_in_force: Option<String>,
    pub(super) order_type: Option<String>,
    pub(super) side: Option<String>,
    pub(super) stop_price: Option<String>,
    pub(super) iceberg_quantity: Option<String>,
    pub(super) created_time: Option<i64>,
    pub(super) updated_time: Option<i64>,
    pub(super) is_working: Option<bool>,
    pub(super) working_time: Option<i64>,
    pub(super) self_trade_prevention_mode: Option<String>,
}

impl From<NewOrderResponse> for BinanceOrderSnapshot {
    fn from(value: NewOrderResponse) -> Self {
        Self {
            symbol: value.symbol,
            order_id: value.order_id,
            order_list_id: value.order_list_id,
            client_order_id: value.client_order_id,
            transaction_time: value.transact_time,
            price: value.price,
            original_quantity: value.orig_qty,
            executed_quantity: value.executed_qty,
            original_quote_quantity: value.orig_quote_order_qty,
            cumulative_quote_quantity: value.cummulative_quote_qty,
            status: value.status,
            time_in_force: value.time_in_force,
            order_type: value.r#type,
            side: value.side,
            working_time: value.working_time,
            self_trade_prevention_mode: value.self_trade_prevention_mode,
            ..Self::default()
        }
    }
}

impl From<DeleteOrderResponse> for BinanceOrderSnapshot {
    fn from(value: DeleteOrderResponse) -> Self {
        Self {
            symbol: value.symbol,
            order_id: value.order_id,
            order_list_id: value.order_list_id,
            client_order_id: value.client_order_id,
            original_client_order_id: value.orig_client_order_id,
            transaction_time: value.transact_time,
            price: value.price,
            original_quantity: value.orig_qty,
            executed_quantity: value.executed_qty,
            original_quote_quantity: value.orig_quote_order_qty,
            cumulative_quote_quantity: value.cummulative_quote_qty,
            status: value.status,
            time_in_force: value.time_in_force,
            order_type: value.r#type,
            side: value.side,
            self_trade_prevention_mode: value.self_trade_prevention_mode,
            ..Self::default()
        }
    }
}

impl From<GetOrderResponse> for BinanceOrderSnapshot {
    fn from(value: GetOrderResponse) -> Self {
        Self {
            symbol: value.symbol,
            order_id: value.order_id,
            order_list_id: value.order_list_id,
            client_order_id: value.client_order_id,
            price: value.price,
            original_quantity: value.orig_qty,
            executed_quantity: value.executed_qty,
            original_quote_quantity: value.orig_quote_order_qty,
            cumulative_quote_quantity: value.cummulative_quote_qty,
            status: value.status,
            time_in_force: value.time_in_force,
            order_type: value.r#type,
            side: value.side,
            stop_price: value.stop_price,
            iceberg_quantity: value.iceberg_qty,
            created_time: value.time,
            updated_time: value.update_time,
            is_working: value.is_working,
            working_time: value.working_time,
            self_trade_prevention_mode: value.self_trade_prevention_mode,
            ..Self::default()
        }
    }
}

impl From<AllOrdersResponseInner> for BinanceOrderSnapshot {
    fn from(value: AllOrdersResponseInner) -> Self {
        Self {
            symbol: value.symbol,
            order_id: value.order_id,
            order_list_id: value.order_list_id,
            client_order_id: value.client_order_id,
            price: value.price,
            original_quantity: value.orig_qty,
            executed_quantity: value.executed_qty,
            original_quote_quantity: value.orig_quote_order_qty,
            cumulative_quote_quantity: value.cummulative_quote_qty,
            status: value.status,
            time_in_force: value.time_in_force,
            order_type: value.r#type,
            side: value.side,
            stop_price: value.stop_price,
            iceberg_quantity: value.iceberg_qty,
            created_time: value.time,
            updated_time: value.update_time,
            is_working: value.is_working,
            working_time: value.working_time,
            self_trade_prevention_mode: value.self_trade_prevention_mode,
            ..Self::default()
        }
    }
}

pub(crate) fn order_from_snapshot(
    snapshot: BinanceOrderSnapshot,
    operation: &'static str,
) -> Result<Order> {
    let raw_status = snapshot
        .status
        .as_deref()
        .ok_or_else(|| crate::error::missing_field(operation, "status"))?;
    let raw_order_type = snapshot
        .order_type
        .as_deref()
        .ok_or_else(|| crate::error::missing_field(operation, "type"))?;
    let mut extensions = Extensions::new();
    internal::insert_order_extensions(&mut extensions, &snapshot, operation)?;
    let side = internal::order_side_from_raw(
        snapshot
            .side
            .as_deref()
            .ok_or_else(|| crate::error::missing_field(operation, "side"))?,
        operation,
    )?;
    let order_type = internal::order_type_from_raw(raw_order_type, operation)?;
    let status = internal::order_status_from_raw(raw_status, operation)?;
    let time_in_force = snapshot
        .time_in_force
        .map(|raw| {
            raw.parse::<mkt_types::TimeInForce>().map_err(|err| {
                crate::error::invalid_field(operation, "timeInForce", err.to_string())
            })
        })
        .transpose()?;
    let order_id = snapshot
        .order_id
        .ok_or_else(|| crate::error::missing_field(operation, "orderId"))?;
    let created_at = internal::parse_required_unix_millis_timestamp(
        snapshot
            .created_time
            .or(snapshot.transaction_time)
            .or(snapshot.working_time),
        operation,
        "time",
    )?;
    let filled_quantity =
        internal::parse_required_decimal(snapshot.executed_quantity, operation, "executedQty")?;
    let original_quote_quantity = internal::parse_optional_decimal(
        snapshot.original_quote_quantity,
        operation,
        "origQuoteOrderQty",
    )?;
    let cumulative_quote_quantity = internal::parse_optional_decimal(
        snapshot.cumulative_quote_quantity,
        operation,
        "cummulativeQuoteQty",
    )?;

    Order::builder()
        .id(OrderId::new(order_id.to_string()))
        .client_order_id(snapshot.client_order_id.map(ClientOrderId::new))
        .symbol(Symbol::spot(snapshot.symbol.ok_or_else(|| {
            crate::error::missing_field(operation, "symbol")
        })?))
        .market_kind(MarketKind::Spot)
        .side(side)
        .order_type(order_type)
        .status(status)
        .time_in_force(time_in_force)
        .price(internal::parse_optional_decimal(
            snapshot.price,
            operation,
            "price",
        )?)
        .quantity(internal::parse_required_decimal(
            snapshot.original_quantity,
            operation,
            "origQty",
        )?)
        .filled_quantity(filled_quantity)
        .original_quote_quantity(original_quote_quantity)
        .cumulative_quote_quantity(cumulative_quote_quantity)
        .created_at(created_at)
        .updated_at(internal::parse_optional_unix_millis_timestamp(
            snapshot.updated_time.or(snapshot.transaction_time),
            operation,
            "updateTime",
        )?)
        .extensions(extensions)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "order", err.to_string()))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use binance_sdk::spot::rest_api::{
        AllOrdersResponseInner, DeleteOrderResponse, GetOrderResponse, NewOrderResponse,
    };
    use mkt_types::{OrderSide, OrderStatus, OrderType};
    use rust_decimal::Decimal;
    use serde_json::Value;

    use super::order_from_snapshot;

    const OPERATION: &str = "spot.workflow.order";

    #[test]
    fn maps_place_cancel_get_and_open_order_payloads_into_stable_orders() {
        let placed = order_from_snapshot(new_order_response().into(), OPERATION)
            .expect("new-order payload should map to a stable Order");
        assert_eq!(placed.id.0, "101");
        assert_eq!(
            placed.client_order_id.as_ref().map(|id| id.0.as_str()),
            Some("c-101")
        );
        assert_eq!(placed.symbol.venue_symbol, "BTCUSDT");
        assert_eq!(placed.side, OrderSide::Buy);
        assert_eq!(placed.order_type, OrderType::Limit);
        assert_eq!(placed.status, OrderStatus::New);
        assert_eq!(placed.price, Some(decimal("43000.50")));
        assert_eq!(placed.quantity, decimal("1.25"));
        assert_eq!(placed.filled_quantity, decimal("0"));
        assert_eq!(placed.created_at.unix_timestamp(), 1_700_000_000);

        let canceled = order_from_snapshot(delete_order_response().into(), OPERATION)
            .expect("delete-order payload should map to a stable Order");
        assert_eq!(canceled.id.0, "102");
        assert_eq!(canceled.status, OrderStatus::Canceled);
        assert_eq!(
            canceled
                .extensions
                .string(crate::ext::ORIGINAL_CLIENT_ORDER_ID)
                .expect("original client order id should be textual"),
            Some("c-102-original".to_owned())
        );

        let queried = order_from_snapshot(get_order_response().into(), OPERATION)
            .expect("get-order payload should map to a stable Order");
        assert_eq!(queried.id.0, "103");
        assert_eq!(queried.order_type, OrderType::StopLimit);
        assert_eq!(queried.status, OrderStatus::PartiallyFilled);
        assert_eq!(
            queried.updated_at.map(|time| time.unix_timestamp()),
            Some(1_700_000_100)
        );
        assert_eq!(
            queried
                .extensions
                .string(crate::ext::STOP_PRICE)
                .expect("stop price extension should be textual"),
            Some("42000".to_owned())
        );
        assert_eq!(
            queried.extensions.get(crate::ext::IS_WORKING),
            Some(&Value::Bool(true))
        );

        let open = order_from_snapshot(open_order_response().into(), OPERATION)
            .expect("open-order payload should map to a stable Order");
        assert_eq!(open.id.0, "104");
        assert_eq!(open.order_type, OrderType::PostOnly);
        assert_eq!(open.status, OrderStatus::New);
    }

    #[test]
    fn rejects_negative_optional_unix_millis_timestamp() {
        let mut response = get_order_response();
        response.update_time = Some(-1);

        let err = order_from_snapshot(response.into(), OPERATION)
            .expect_err("negative optional Unix millisecond timestamp must not be ignored");

        assert!(err.to_string().contains("updateTime"));
        assert!(err
            .to_string()
            .contains("invalid Unix millisecond timestamp"));
    }

    fn new_order_response() -> NewOrderResponse {
        let mut response = NewOrderResponse::new();
        response.symbol = Some("BTCUSDT".to_owned());
        response.order_id = Some(101);
        response.order_list_id = Some(-1);
        response.client_order_id = Some("c-101".to_owned());
        response.transact_time = Some(1_700_000_000_123);
        response.price = Some("43000.50".to_owned());
        response.orig_qty = Some("1.25".to_owned());
        response.executed_qty = Some("0".to_owned());
        response.orig_quote_order_qty = Some("0".to_owned());
        response.cummulative_quote_qty = Some("0".to_owned());
        response.status = Some("NEW".to_owned());
        response.time_in_force = Some("GTC".to_owned());
        response.r#type = Some("LIMIT".to_owned());
        response.side = Some("BUY".to_owned());
        response.working_time = Some(1_700_000_000_123);
        response.self_trade_prevention_mode = Some("NONE".to_owned());
        response
    }

    fn delete_order_response() -> DeleteOrderResponse {
        let mut response = DeleteOrderResponse::new();
        response.symbol = Some("BTCUSDT".to_owned());
        response.orig_client_order_id = Some("c-102-original".to_owned());
        response.order_id = Some(102);
        response.order_list_id = Some(-1);
        response.client_order_id = Some("c-102-cancel".to_owned());
        response.transact_time = Some(1_700_000_050_000);
        response.price = Some("43000.50".to_owned());
        response.orig_qty = Some("1.25".to_owned());
        response.executed_qty = Some("0.25".to_owned());
        response.orig_quote_order_qty = Some("0".to_owned());
        response.cummulative_quote_qty = Some("10750.125".to_owned());
        response.status = Some("CANCELED".to_owned());
        response.time_in_force = Some("GTC".to_owned());
        response.r#type = Some("LIMIT".to_owned());
        response.side = Some("SELL".to_owned());
        response.self_trade_prevention_mode = Some("NONE".to_owned());
        response
    }

    fn get_order_response() -> GetOrderResponse {
        let mut response = GetOrderResponse::new();
        response.symbol = Some("ETHUSDT".to_owned());
        response.order_id = Some(103);
        response.order_list_id = Some(-1);
        response.client_order_id = Some("c-103".to_owned());
        response.price = Some("43000.50".to_owned());
        response.orig_qty = Some("1.25".to_owned());
        response.executed_qty = Some("0.25".to_owned());
        response.cummulative_quote_qty = Some("10750.125".to_owned());
        response.status = Some("PARTIALLY_FILLED".to_owned());
        response.time_in_force = Some("GTC".to_owned());
        response.r#type = Some("STOP_LOSS_LIMIT".to_owned());
        response.side = Some("SELL".to_owned());
        response.stop_price = Some("42000".to_owned());
        response.iceberg_qty = Some("0.10".to_owned());
        response.time = Some(1_700_000_000_000);
        response.update_time = Some(1_700_000_100_000);
        response.is_working = Some(true);
        response.working_time = Some(1_700_000_000_100);
        response.orig_quote_order_qty = Some("0".to_owned());
        response.self_trade_prevention_mode = Some("NONE".to_owned());
        response
    }

    fn open_order_response() -> AllOrdersResponseInner {
        let mut response = AllOrdersResponseInner::new();
        response.symbol = Some("BNBUSDT".to_owned());
        response.order_id = Some(104);
        response.order_list_id = Some(-1);
        response.client_order_id = Some("c-104".to_owned());
        response.price = Some("600".to_owned());
        response.orig_qty = Some("2".to_owned());
        response.executed_qty = Some("0".to_owned());
        response.cummulative_quote_qty = Some("0".to_owned());
        response.status = Some("NEW".to_owned());
        response.time_in_force = Some("GTC".to_owned());
        response.r#type = Some("LIMIT_MAKER".to_owned());
        response.side = Some("BUY".to_owned());
        response.time = Some(1_700_000_200_000);
        response.update_time = Some(1_700_000_200_000);
        response.is_working = Some(true);
        response.orig_quote_order_qty = Some("0".to_owned());
        response.working_time = Some(1_700_000_200_000);
        response.self_trade_prevention_mode = Some("NONE".to_owned());
        response
    }

    fn decimal(raw: &str) -> Decimal {
        Decimal::from_str(raw).expect("decimal test literal must be valid")
    }
}
