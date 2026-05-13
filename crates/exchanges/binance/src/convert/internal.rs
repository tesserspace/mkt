use std::str::FromStr;

use binance_sdk::spot::rest_api::{NewOrderSideEnum, NewOrderTimeInForceEnum, NewOrderTypeEnum};
use mkt_core::Result;
use mkt_types::{Extensions, OrderSide, OrderStatus, OrderType, SpotOrderRequest, TimeInForce};
use rust_decimal::Decimal;
use time::OffsetDateTime;

use super::order::BinanceOrderSnapshot;

pub(super) fn validate_new_order_request(
    request: &SpotOrderRequest,
    order_type: NewOrderTypeEnum,
    stop_price: Option<Decimal>,
    operation: &'static str,
) -> Result<()> {
    let has_quantity = request.quantity.map(|value| value > Decimal::ZERO).unwrap_or(false);
    let has_quote_quantity = request
        .quote_quantity
        .map(|value| value > Decimal::ZERO)
        .unwrap_or(false);

    if has_quantity == has_quote_quantity {
        return Err(crate::error::invalid_field(
            operation,
            "quantity",
            "exactly one of quantity or quote_quantity must be greater than zero",
        ));
    }

    match order_type {
        NewOrderTypeEnum::Market if has_quote_quantity && request.side != OrderSide::Buy => {
            Err(crate::error::invalid_field(
                operation,
                "quote_quantity",
                "quote_quantity is only supported for Binance spot market buys",
            ))
        }
        NewOrderTypeEnum::Limit
        | NewOrderTypeEnum::LimitMaker
        | NewOrderTypeEnum::StopLoss
        | NewOrderTypeEnum::TakeProfit
        | NewOrderTypeEnum::StopLossLimit
        | NewOrderTypeEnum::TakeProfitLimit
            if has_quote_quantity =>
        {
            Err(crate::error::invalid_field(
                operation,
                "quote_quantity",
                "quote_quantity is only supported for Binance spot market buys",
            ))
        }
        NewOrderTypeEnum::Market if request.price.is_some() => Err(crate::error::invalid_field(
            operation,
            "price",
            "market orders must not carry a limit price",
        )),
        NewOrderTypeEnum::Limit | NewOrderTypeEnum::LimitMaker if request.price.is_none() => {
            Err(crate::error::invalid_field(
                operation,
                "price",
                "price is required for limit-style orders",
            ))
        }
        NewOrderTypeEnum::StopLoss | NewOrderTypeEnum::TakeProfit if stop_price.is_none() => {
            Err(crate::error::invalid_field(
                operation,
                crate::ext::STOP_PRICE,
                "stop orders require `binance.stop_price`",
            ))
        }
        NewOrderTypeEnum::StopLossLimit | NewOrderTypeEnum::TakeProfitLimit
            if request.price.is_none() =>
        {
            Err(crate::error::invalid_field(
                operation,
                "price",
                "stop-limit orders require a limit price",
            ))
        }
        NewOrderTypeEnum::StopLossLimit | NewOrderTypeEnum::TakeProfitLimit
            if stop_price.is_none() =>
        {
            Err(crate::error::invalid_field(
                operation,
                crate::ext::STOP_PRICE,
                "stop-limit orders require `binance.stop_price`",
            ))
        }
        NewOrderTypeEnum::Market
        | NewOrderTypeEnum::Limit
        | NewOrderTypeEnum::LimitMaker
        | NewOrderTypeEnum::StopLoss
        | NewOrderTypeEnum::TakeProfit
        | NewOrderTypeEnum::StopLossLimit
        | NewOrderTypeEnum::TakeProfitLimit => Ok(()),
        NewOrderTypeEnum::NonRepresentable => Err(crate::error::invalid_field(
            operation,
            "type",
            "unsupported Binance order type",
        )),
    }
}

pub(super) fn resolve_order_type(
    request: &SpotOrderRequest,
    operation: &'static str,
) -> Result<NewOrderTypeEnum> {
    match request.order_type {
        OrderType::Market => Ok(NewOrderTypeEnum::Market),
        OrderType::Limit => Ok(NewOrderTypeEnum::Limit),
        OrderType::StopMarket => Ok(NewOrderTypeEnum::StopLoss),
        OrderType::StopLimit => Ok(NewOrderTypeEnum::StopLossLimit),
        OrderType::PostOnly => Ok(NewOrderTypeEnum::LimitMaker),
        _ => Err(crate::error::invalid_field(
            operation,
            "type",
            "unsupported request order type",
        )),
    }
}

pub(super) fn resolve_time_in_force(
    request: &SpotOrderRequest,
    operation: &'static str,
) -> Result<Option<NewOrderTimeInForceEnum>> {
    match request.order_type {
        OrderType::Limit | OrderType::StopLimit => Ok(Some(to_sdk_time_in_force(
            request.time_in_force.unwrap_or(TimeInForce::Gtc),
            operation,
        )?)),
        OrderType::Market | OrderType::StopMarket | OrderType::PostOnly => Ok(None),
        _ => Err(crate::error::invalid_field(
            operation,
            "time_in_force",
            "unsupported request order type",
        )),
    }
}

pub(super) fn to_sdk_side(side: OrderSide, operation: &'static str) -> Result<NewOrderSideEnum> {
    match side {
        OrderSide::Buy => Ok(NewOrderSideEnum::Buy),
        OrderSide::Sell => Ok(NewOrderSideEnum::Sell),
        _ => Err(crate::error::invalid_field(
            operation,
            "side",
            "unsupported order side",
        )),
    }
}

fn to_sdk_time_in_force(
    time_in_force: TimeInForce,
    operation: &'static str,
) -> Result<NewOrderTimeInForceEnum> {
    match time_in_force {
        TimeInForce::Gtc => Ok(NewOrderTimeInForceEnum::Gtc),
        TimeInForce::Ioc => Ok(NewOrderTimeInForceEnum::Ioc),
        TimeInForce::Fok => Ok(NewOrderTimeInForceEnum::Fok),
        TimeInForce::Gtx => Err(crate::error::invalid_field(
            operation,
            "time_in_force",
            "Binance spot does not accept GTX; use `OrderType::PostOnly` instead",
        )),
        _ => Err(crate::error::invalid_field(
            operation,
            "time_in_force",
            "unsupported time-in-force",
        )),
    }
}

pub(super) fn parse_required_decimal(
    raw: Option<String>,
    operation: &'static str,
    field: &'static str,
) -> Result<Decimal> {
    Decimal::from_str(
        raw.ok_or_else(|| crate::error::missing_field(operation, field))?
            .as_str(),
    )
    .map_err(|err| crate::error::invalid_field(operation, field, err.to_string()))
}

pub(super) fn parse_optional_decimal(
    raw: Option<String>,
    operation: &'static str,
    field: &'static str,
) -> Result<Option<Decimal>> {
    raw.map(|value| {
        Decimal::from_str(value.as_str())
            .map_err(|err| crate::error::invalid_field(operation, field, err.to_string()))
    })
    .transpose()
}

pub(super) fn parse_required_unix_millis_timestamp(
    raw: Option<i64>,
    operation: &'static str,
    field: &'static str,
) -> Result<OffsetDateTime> {
    parse_unix_millis_timestamp(
        raw.ok_or_else(|| crate::error::missing_field(operation, field))?,
        operation,
        field,
    )
}

pub(super) fn parse_optional_unix_millis_timestamp(
    raw: Option<i64>,
    operation: &'static str,
    field: &'static str,
) -> Result<Option<OffsetDateTime>> {
    raw.map(|timestamp_millis| parse_unix_millis_timestamp(timestamp_millis, operation, field))
        .transpose()
}

pub(super) fn parse_unix_millis_timestamp(
    timestamp_millis: i64,
    operation: &'static str,
    field: &'static str,
) -> Result<OffsetDateTime> {
    if timestamp_millis < 0 {
        return Err(crate::error::invalid_field(
            operation,
            field,
            "invalid Unix millisecond timestamp",
        ));
    }

    OffsetDateTime::from_unix_timestamp_nanos(i128::from(timestamp_millis) * 1_000_000).map_err(
        |_| crate::error::invalid_field(operation, field, "invalid Unix millisecond timestamp"),
    )
}

pub(super) fn insert_order_extensions(
    extensions: &mut Extensions,
    snapshot: &BinanceOrderSnapshot,
    operation: &'static str,
) -> Result<()> {
    extensions
        .insert_optional_string(
            crate::ext::ORIGINAL_CLIENT_ORDER_ID,
            snapshot.original_client_order_id.clone(),
        )
        .map_err(|err| {
            crate::error::invalid_field(
                operation,
                crate::ext::ORIGINAL_CLIENT_ORDER_ID,
                err.to_string(),
            )
        })?;
    extensions
        .insert_optional_string(crate::ext::STOP_PRICE, snapshot.stop_price.clone())
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::STOP_PRICE, err.to_string())
        })?;
    extensions
        .insert_optional_string(
            crate::ext::ICEBERG_QUANTITY,
            snapshot.iceberg_quantity.clone(),
        )
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::ICEBERG_QUANTITY, err.to_string())
        })?;
    extensions
        .insert_optional_string(
            crate::ext::SELF_TRADE_PREVENTION_MODE,
            snapshot.self_trade_prevention_mode.clone(),
        )
        .map_err(|err| {
            crate::error::invalid_field(
                operation,
                crate::ext::SELF_TRADE_PREVENTION_MODE,
                err.to_string(),
            )
        })?;
    extensions
        .insert_optional_bool(crate::ext::IS_WORKING, snapshot.is_working)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::IS_WORKING, err.to_string())
        })?;
    if let Some(order_list_id) = snapshot.order_list_id {
        extensions
            .insert_i64(crate::ext::ORDER_LIST_ID, order_list_id)
            .map_err(|err| {
                crate::error::invalid_field(operation, crate::ext::ORDER_LIST_ID, err.to_string())
            })?;
    }
    if let Some(working_time) = snapshot.working_time {
        extensions
            .insert_i64(crate::ext::WORKING_TIME, working_time)
            .map_err(|err| {
                crate::error::invalid_field(operation, crate::ext::WORKING_TIME, err.to_string())
            })?;
    }
    Ok(())
}

pub(super) fn order_side_from_raw(raw: &str, operation: &'static str) -> Result<OrderSide> {
    OrderSide::from_str(raw)
        .map_err(|err| crate::error::invalid_field(operation, "side", err.to_string()))
}

pub(super) fn order_status_from_raw(raw: &str, operation: &'static str) -> Result<OrderStatus> {
    OrderStatus::from_str(raw)
        .map_err(|err| crate::error::invalid_field(operation, "status", err.to_string()))
}

pub(super) fn order_type_from_raw(raw: &str, operation: &'static str) -> Result<OrderType> {
    let order_type = NewOrderTypeEnum::from_str(raw)
        .map_err(|err| crate::error::invalid_field(operation, "type", err.to_string()))?;

    match order_type {
        NewOrderTypeEnum::Market => Ok(OrderType::Market),
        NewOrderTypeEnum::Limit => Ok(OrderType::Limit),
        NewOrderTypeEnum::StopLoss | NewOrderTypeEnum::TakeProfit => Ok(OrderType::StopMarket),
        NewOrderTypeEnum::StopLossLimit | NewOrderTypeEnum::TakeProfitLimit => {
            Ok(OrderType::StopLimit)
        }
        NewOrderTypeEnum::LimitMaker => Ok(OrderType::PostOnly),
        NewOrderTypeEnum::NonRepresentable => Err(crate::error::invalid_field(
            operation,
            "type",
            "unsupported Binance order type",
        )),
    }
}
