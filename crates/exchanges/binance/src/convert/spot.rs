use std::str::FromStr;

use binance_sdk::spot::rest_api::{
    GetAccountResponseBalancesInner, KlinesIntervalEnum, KlinesParams, MyTradesResponseInner,
    NewOrderParams,
};
use mkt_core::Result;
use mkt_types::{Balance, Fill, KlineInterval, KlineRequest, OrderKey, OrderQuantity, SpotOrderRequest};

use super::internal;

pub(crate) fn build_new_order_params(
    request: &SpotOrderRequest,
    operation: &'static str,
) -> Result<NewOrderParams> {
    let symbol = require_spot_symbol(&request.symbol, operation)?;
    let order_type = internal::resolve_order_type(request, operation)?;
    let time_in_force = internal::resolve_time_in_force(request, operation)?;
    let stop_price = request
        .extensions
        .decimal(crate::ext::STOP_PRICE)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::STOP_PRICE, err.to_string())
        })?;
    let iceberg_quantity = request
        .extensions
        .decimal(crate::ext::ICEBERG_QUANTITY)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::ICEBERG_QUANTITY, err.to_string())
        })?;
    let strategy_id = request
        .extensions
        .i64(crate::ext::STRATEGY_ID)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::STRATEGY_ID, err.to_string())
        })?;
    let strategy_type = request
        .extensions
        .i32(crate::ext::STRATEGY_TYPE)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::STRATEGY_TYPE, err.to_string())
        })?;
    let trailing_delta = request
        .extensions
        .i64(crate::ext::TRAILING_DELTA)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::TRAILING_DELTA, err.to_string())
        })?;
    let recv_window = request
        .extensions
        .decimal(crate::ext::RECV_WINDOW)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::RECV_WINDOW, err.to_string())
        })?;
    let self_trade_prevention_mode = request
        .extensions
        .string(crate::ext::SELF_TRADE_PREVENTION_MODE)
        .map_err(|err| {
            crate::error::invalid_field(
                operation,
                crate::ext::SELF_TRADE_PREVENTION_MODE,
                err.to_string(),
            )
        })?
        .map(|value| {
            binance_sdk::spot::rest_api::NewOrderSelfTradePreventionModeEnum::from_str(&value)
                .map_err(|err| {
                    crate::error::invalid_field(
                        operation,
                        crate::ext::SELF_TRADE_PREVENTION_MODE,
                        err.to_string(),
                    )
                })
        })
        .transpose()?;
    internal::validate_new_order_request(request, order_type.clone(), stop_price, operation)?;
    let mut builder = NewOrderParams::builder(
        symbol,
        internal::to_sdk_side(request.side, operation)?,
        order_type,
    )
    .new_order_resp_type(binance_sdk::spot::rest_api::NewOrderNewOrderRespTypeEnum::Full);
    match request.quantity {
        OrderQuantity::Base(quantity) => {
            builder = builder.quantity(quantity);
        }
        OrderQuantity::Quote(quote_quantity) => {
            builder = builder.quote_order_qty(quote_quantity);
        }
        _ => {
            return Err(crate::error::invalid_field(
                operation,
                "quantity",
                "unsupported quantity mode for Binance spot",
            ))
        }
    }
    if let Some(client_order_id) = &request.client_order_id {
        builder = builder.new_client_order_id(client_order_id.0.clone());
    }
    if let Some(time_in_force) = time_in_force {
        builder = builder.time_in_force(time_in_force);
    }
    if let Some(price) = request.price {
        builder = builder.price(price);
    }
    if let Some(stop_price) = stop_price {
        builder = builder.stop_price(stop_price);
    }
    if let Some(iceberg_quantity) = iceberg_quantity {
        builder = builder.iceberg_qty(iceberg_quantity);
    }
    if let Some(strategy_id) = strategy_id {
        builder = builder.strategy_id(strategy_id);
    }
    if let Some(strategy_type) = strategy_type {
        builder = builder.strategy_type(strategy_type);
    }
    if let Some(trailing_delta) = trailing_delta {
        builder = builder.trailing_delta(trailing_delta);
    }
    if let Some(recv_window) = recv_window {
        builder = builder.recv_window(recv_window);
    }
    if let Some(mode) = self_trade_prevention_mode {
        builder = builder.self_trade_prevention_mode(mode);
    }
    builder
        .build()
        .map_err(|err| crate::error::adapter_error(operation, err.to_string()))
}

pub(crate) fn build_klines_params(
    request: &KlineRequest,
    operation: &'static str,
) -> Result<KlinesParams> {
    let interval = match request.interval {
        KlineInterval::Second(1) => KlinesIntervalEnum::Interval1s,
        KlineInterval::Minute(1) => KlinesIntervalEnum::Interval1m,
        KlineInterval::Minute(3) => KlinesIntervalEnum::Interval3m,
        KlineInterval::Minute(5) => KlinesIntervalEnum::Interval5m,
        KlineInterval::Minute(15) => KlinesIntervalEnum::Interval15m,
        KlineInterval::Minute(30) => KlinesIntervalEnum::Interval30m,
        KlineInterval::Hour(1) => KlinesIntervalEnum::Interval1h,
        KlineInterval::Hour(2) => KlinesIntervalEnum::Interval2h,
        KlineInterval::Hour(4) => KlinesIntervalEnum::Interval4h,
        KlineInterval::Hour(6) => KlinesIntervalEnum::Interval6h,
        KlineInterval::Hour(8) => KlinesIntervalEnum::Interval8h,
        KlineInterval::Hour(12) => KlinesIntervalEnum::Interval12h,
        KlineInterval::Day(1) => KlinesIntervalEnum::Interval1d,
        KlineInterval::Day(3) => KlinesIntervalEnum::Interval3d,
        KlineInterval::Week(1) => KlinesIntervalEnum::Interval1w,
        KlineInterval::Month(1) => KlinesIntervalEnum::Interval1M,
        _ => {
            return Err(crate::error::invalid_field(
                operation,
                "interval",
                "unsupported Binance spot kline interval",
            ))
        }
    };
    let to_millis = |timestamp: time::OffsetDateTime, field: &'static str| -> Result<i64> {
        i64::try_from(timestamp.unix_timestamp_nanos() / 1_000_000).map_err(|_| {
            crate::error::invalid_field(
                operation,
                field,
                "timestamp is out of i64 millisecond range",
            )
        })
    };

    let mut builder =
        KlinesParams::builder(require_spot_symbol(&request.symbol, operation)?, interval);
    if let Some(start) = request.start {
        builder = builder.start_time(to_millis(start, "start")?);
    }
    if let Some(end) = request.end {
        builder = builder.end_time(to_millis(end, "end")?);
    }
    if let Some(limit) = request.limit {
        builder = builder.limit(i32::try_from(limit).map_err(|_| {
            crate::error::invalid_field(
                operation,
                "limit",
                "limit is out of i32 range for Binance spot",
            )
        })?);
    }

    builder
        .build()
        .map_err(|err| crate::error::adapter_error(operation, err.to_string()))
}

pub(crate) fn require_spot_symbol(
    symbol: &mkt_types::Symbol,
    operation: &'static str,
) -> Result<String> {
    if !matches!(symbol.kind, mkt_types::MarketKind::Spot) {
        return Err(crate::error::invalid_field(
            operation,
            "symbol",
            format!(
                "Binance spot workflow only accepts spot symbols, got `{}`",
                symbol.kind
            ),
        ));
    }
    Ok(symbol.venue_symbol.clone())
}

pub(crate) fn lookup_order_key(
    key: &OrderKey,
    operation: &'static str,
) -> Result<(Option<i64>, Option<String>)> {
    match key {
        OrderKey::Exchange(order_id) => Ok((
            Some(parse_exchange_order_id(order_id.0.as_str(), operation)?),
            None,
        )),
        OrderKey::Client(client_order_id) => Ok((None, Some(client_order_id.0.clone()))),
        _ => Err(crate::error::invalid_field(
            operation,
            "key",
            "unsupported order key variant",
        )),
    }
}

pub(crate) fn parse_exchange_order_id(raw: &str, operation: &'static str) -> Result<i64> {
    raw.parse::<i64>()
        .map_err(|err| crate::error::invalid_field(operation, "order_id", err.to_string()))
}

pub(crate) fn fill_from_trade(
    trade: MyTradesResponseInner,
    operation: &'static str,
) -> Result<Fill> {
    let symbol = mkt_types::Symbol::spot(
        trade
            .symbol
            .ok_or_else(|| crate::error::missing_field(operation, "symbol"))?,
    );
    let order_id = mkt_types::OrderId::new(
        trade
            .order_id
            .ok_or_else(|| crate::error::missing_field(operation, "orderId"))?
            .to_string(),
    );
    let side = match trade.is_buyer {
        Some(true) => mkt_types::OrderSide::Buy,
        Some(false) => mkt_types::OrderSide::Sell,
        None => return Err(crate::error::missing_field(operation, "isBuyer")),
    };
    let price = rust_decimal::Decimal::from_str(
        trade
            .price
            .ok_or_else(|| crate::error::missing_field(operation, "price"))?
            .as_str(),
    )
    .map_err(|err| crate::error::invalid_field(operation, "price", err.to_string()))?;
    let quantity = rust_decimal::Decimal::from_str(
        trade
            .qty
            .ok_or_else(|| crate::error::missing_field(operation, "qty"))?
            .as_str(),
    )
    .map_err(|err| crate::error::invalid_field(operation, "qty", err.to_string()))?;
    let quote_quantity = match trade.quote_qty {
        Some(raw) => Some(
            rust_decimal::Decimal::from_str(raw.as_str())
                .map_err(|err| crate::error::invalid_field(operation, "quoteQty", err.to_string()))?,
        ),
        None => None,
    };
    let timestamp_ms = trade
        .time
        .ok_or_else(|| crate::error::missing_field(operation, "time"))?;
    let timestamp = internal::parse_unix_millis_timestamp(timestamp_ms, operation, "time")?;
    let mut extensions = mkt_types::Extensions::new();
    extensions
        .insert_optional_bool(crate::ext::IS_MAKER, trade.is_maker)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::IS_MAKER, err.to_string())
        })?;
    extensions
        .insert_optional_bool(crate::ext::IS_BEST_MATCH, trade.is_best_match)
        .map_err(|err| {
            crate::error::invalid_field(operation, crate::ext::IS_BEST_MATCH, err.to_string())
        })?;
    if let Some(order_list_id) = trade.order_list_id {
        extensions
            .insert_i64(crate::ext::ORDER_LIST_ID, order_list_id)
            .map_err(|err| {
                crate::error::invalid_field(operation, crate::ext::ORDER_LIST_ID, err.to_string())
            })?;
    }
    Fill::builder()
        .id(trade.id.map(|value| value.to_string()))
        .order_id(order_id)
        .symbol(symbol)
        .side(side)
        .price(price)
        .quantity(quantity)
        .quote_quantity(quote_quantity)
        .fee(match trade.commission {
            Some(raw) => Some(
                rust_decimal::Decimal::from_str(raw.as_str()).map_err(|err| {
                    crate::error::invalid_field(operation, "commission", err.to_string())
                })?,
            ),
            None => None,
        })
        .fee_asset(trade.commission_asset)
        .timestamp(timestamp)
        .extensions(extensions)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "fill", err.to_string()))
}

pub(crate) fn balance_from_account_balance(
    balance: GetAccountResponseBalancesInner,
    account_update_time_ms: i64,
    operation: &'static str,
) -> Result<Balance> {
    let available = rust_decimal::Decimal::from_str(
        balance
            .free
            .ok_or_else(|| crate::error::missing_field(operation, "free"))?
            .as_str(),
    )
    .map_err(|err| crate::error::invalid_field(operation, "free", err.to_string()))?;
    let locked = rust_decimal::Decimal::from_str(
        balance
            .locked
            .ok_or_else(|| crate::error::missing_field(operation, "locked"))?
            .as_str(),
    )
    .map_err(|err| crate::error::invalid_field(operation, "locked", err.to_string()))?;
    Balance::builder()
        .asset(
            balance
                .asset
                .ok_or_else(|| crate::error::missing_field(operation, "asset"))?,
        )
        .available(available)
        .locked(locked)
        .total(available + locked)
        .timestamp(internal::parse_unix_millis_timestamp(
            account_update_time_ms,
            operation,
            "updateTime",
        )?)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "balance", err.to_string()))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use binance_sdk::spot::rest_api::{
        GetAccountResponseBalancesInner, MyTradesResponseInner, NewOrderSideEnum,
        NewOrderTimeInForceEnum, NewOrderTypeEnum,
    };
    use mkt_types::{
        ClientOrderId, DerivativeKind, Extensions, OrderKey, OrderQuantity, OrderSide, OrderType,
        SettlementMode, SpotOrderRequest, Symbol, TimeInForce,
    };
    use rust_decimal::Decimal;
    use serde_json::Value;

    use super::{
        balance_from_account_balance, build_new_order_params, fill_from_trade, lookup_order_key,
    };

    const OPERATION: &str = "spot.workflow.test";

    #[test]
    fn builds_limit_order_params_from_unified_spot_request() {
        let mut extensions = Extensions::new();
        extensions
            .insert(
                crate::ext::RECV_WINDOW,
                Value::String("5000.123".to_owned()),
            )
            .expect("test extension key is a reviewed Binance key");

        let request = SpotOrderRequest::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .side(OrderSide::Buy)
            .order_type(OrderType::Limit)
            .quantity(OrderQuantity::Base(decimal("1.25")))
            .price(Some(decimal("43000.50")))
            .time_in_force(Some(TimeInForce::Ioc))
            .client_order_id(Some(ClientOrderId::new("client-1")))
            .extensions(extensions)
            .build()
            .expect("complete SpotOrderRequest fixture should build");

        let params = build_new_order_params(&request, OPERATION)
            .expect("valid limit order fixture should convert to Binance params");

        assert_eq!(params.symbol, "BTCUSDT");
        assert!(matches!(params.side, NewOrderSideEnum::Buy));
        assert!(matches!(params.r#type, NewOrderTypeEnum::Limit));
        assert!(matches!(
            params.time_in_force,
            Some(NewOrderTimeInForceEnum::Ioc)
        ));
        assert_eq!(params.quantity, Some(decimal("1.25")));
        assert_eq!(params.price, Some(decimal("43000.50")));
        assert_eq!(params.new_client_order_id.as_deref(), Some("client-1"));
        assert_eq!(params.recv_window, Some(decimal("5000.123")));
    }

    #[test]
    fn builds_market_buy_quote_order_params_from_unified_spot_request() {
        let request = SpotOrderRequest::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(OrderQuantity::Quote(decimal("100")))
            .build()
            .expect("quote market buy fixture should build");

        let params = build_new_order_params(&request, OPERATION)
            .expect("quote market buy should convert to Binance params");

        assert_eq!(params.quantity, None);
        assert_eq!(params.quote_order_qty, Some(decimal("100")));
    }

    #[test]
    fn rejects_invalid_spot_order_requests_before_transport() {
        let market_with_price = SpotOrderRequest::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(OrderQuantity::Base(decimal("1")))
            .price(Some(decimal("43000")))
            .build()
            .expect("market order fixture should satisfy unified request invariants");

        let err = build_new_order_params(&market_with_price, OPERATION)
            .expect_err("market orders with a limit price must be rejected locally");
        assert!(err.to_string().contains("market orders must not carry"));

        let derivative_symbol =
            Symbol::derivative(DerivativeKind::perpetual(SettlementMode::Linear), "BTCUSDT");
        let derivative_request = SpotOrderRequest::builder()
            .symbol(derivative_symbol)
            .side(OrderSide::Buy)
            .order_type(OrderType::Market)
            .quantity(OrderQuantity::Base(decimal("1")))
            .build()
            .expect("derivative-symbol fixture should satisfy unified request invariants");

        let err = build_new_order_params(&derivative_request, OPERATION)
            .expect_err("spot workflow must reject derivative symbols locally");
        assert!(err.to_string().contains("only accepts spot symbols"));

        let invalid_quote_sell = SpotOrderRequest::builder()
            .symbol(Symbol::spot("BTCUSDT"))
            .side(OrderSide::Sell)
            .order_type(OrderType::Market)
            .quantity(OrderQuantity::Quote(decimal("100")))
            .build()
            .expect("quote sell fixture should satisfy unified request invariants");

        let err = build_new_order_params(&invalid_quote_sell, OPERATION)
            .expect_err("quote quantity should be rejected for sells");
        assert!(err.to_string().contains("quote_quantity is only supported"));
    }

    #[test]
    fn maps_fills_balances_and_order_keys_for_spot_workflow() {
        let mut trade = MyTradesResponseInner::new();
        trade.symbol = Some("ETHUSDT".to_owned());
        trade.id = Some(9001);
        trade.order_id = Some(42);
        trade.order_list_id = Some(-1);
        trade.price = Some("2500.25".to_owned());
        trade.qty = Some("0.4".to_owned());
        trade.quote_qty = Some("1000.10".to_owned());
        trade.commission = Some("0.0004".to_owned());
        trade.commission_asset = Some("ETH".to_owned());
        trade.time = Some(1_700_000_000_123);
        trade.is_buyer = Some(false);
        trade.is_maker = Some(true);
        trade.is_best_match = Some(true);

        let fill = fill_from_trade(trade, OPERATION)
            .expect("complete Binance trade fixture should convert to Fill");
        assert_eq!(fill.id.as_deref(), Some("9001"));
        assert_eq!(fill.order_id.0, "42");
        assert_eq!(fill.symbol.venue_symbol, "ETHUSDT");
        assert_eq!(fill.side, OrderSide::Sell);
        assert_eq!(fill.price, decimal("2500.25"));
        assert_eq!(fill.quantity, decimal("0.4"));
        assert_eq!(fill.quote_quantity, Some(decimal("1000.10")));
        assert_eq!(fill.fee, Some(decimal("0.0004")));
        assert_eq!(
            fill.extensions.get(crate::ext::IS_MAKER),
            Some(&Value::Bool(true))
        );

        let mut balance = GetAccountResponseBalancesInner::new();
        balance.asset = Some("USDT".to_owned());
        balance.free = Some("10.5".to_owned());
        balance.locked = Some("1.25".to_owned());
        let balance = balance_from_account_balance(balance, 1_700_000_000_000, OPERATION)
            .expect("complete Binance balance fixture should convert to Balance");
        assert_eq!(balance.asset, "USDT");
        assert_eq!(balance.available, decimal("10.5"));
        assert_eq!(balance.locked, decimal("1.25"));
        assert_eq!(balance.total, decimal("11.75"));
        assert_eq!(balance.timestamp.unix_timestamp(), 1_700_000_000);

        let (exchange_id, client_id) = lookup_order_key(
            &OrderKey::Exchange(mkt_types::OrderId::new("123")),
            OPERATION,
        )
        .expect("numeric exchange order key should map to Binance orderId");
        assert_eq!((exchange_id, client_id), (Some(123), None));
    }

    fn decimal(raw: &str) -> Decimal {
        Decimal::from_str(raw).expect("decimal test literal must be valid")
    }
}
