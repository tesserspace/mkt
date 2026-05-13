use std::str::FromStr;

use binance_sdk::spot::rest_api::{
    ExchangeInfoResponseSymbolsInner, ExchangeInfoSymbolStatusEnum, HistoricalTradesResponseInner,
    KlinesItemInner, SymbolFilters, TickerPriceResponse,
};
use mkt_core::Result;
use mkt_types::{
    ExchangeId, Kline, KlineInterval, KnownExchange, LastPrice, LotSizeFilter, MarketInfo,
    MarketQuantityMode, MarketStatus, NotionalConstraints, OrderBook, OrderBookLevel, PriceFilter,
    QuantityModeSupport, Symbol, Trade, TradeSide, TradingConstraints, TradingPermissions,
};
use rust_decimal::Decimal;
use time::OffsetDateTime;

use super::internal;

pub(crate) fn market_info_from_exchange_symbol(
    symbol_definition: ExchangeInfoResponseSymbolsInner,
    operation: &'static str,
) -> Result<MarketInfo> {
    let status = match ExchangeInfoSymbolStatusEnum::from_str(
        symbol_definition
            .status
            .as_deref()
            .ok_or_else(|| crate::error::missing_field(operation, "status"))?,
    )
    .map_err(|err| crate::error::invalid_field(operation, "status", err.to_string()))?
    {
        ExchangeInfoSymbolStatusEnum::Trading => MarketStatus::Trading,
        ExchangeInfoSymbolStatusEnum::EndOfDay | ExchangeInfoSymbolStatusEnum::Halt => {
            MarketStatus::Halted
        }
        ExchangeInfoSymbolStatusEnum::Break => MarketStatus::PreLaunch,
        ExchangeInfoSymbolStatusEnum::NonRepresentable => {
            return Err(crate::error::invalid_field(
                operation,
                "status",
                "unsupported Binance symbol status",
            ))
        }
    };

    let mut allowed_order_types = Vec::new();
    for raw_order_type in symbol_definition.order_types.unwrap_or_default() {
        let order_type = internal::order_type_from_raw(raw_order_type.as_str(), operation)?;
        if !allowed_order_types.contains(&order_type) {
            allowed_order_types.push(order_type);
        }
    }

    let mut price_filter = None;
    let mut lot_size = None;
    let mut market_lot_size = None;
    let mut min_notional = None;
    let mut max_notional = None;
    let mut apply_min_to_market = None;
    let mut apply_max_to_market = None;

    for filter in symbol_definition.filters.unwrap_or_default() {
        match filter {
            SymbolFilters::PriceFilter(filter_definition) => {
                price_filter = Some(
                    PriceFilter::builder()
                        .min_price(internal::parse_optional_decimal(
                            filter_definition.min_price.clone(),
                            operation,
                            "minPrice",
                        )?)
                        .max_price(internal::parse_optional_decimal(
                            filter_definition.max_price.clone(),
                            operation,
                            "maxPrice",
                        )?)
                        .tick_size(internal::parse_optional_decimal(
                            filter_definition.tick_size.clone(),
                            operation,
                            "tickSize",
                        )?)
                        .build()
                        .map_err(|err| {
                            crate::error::invalid_field(operation, "PRICE_FILTER", err.to_string())
                        })?,
                );
            }
            SymbolFilters::LotSize(filter_definition) => {
                lot_size = Some(
                    LotSizeFilter::builder()
                        .min_quantity(internal::parse_optional_decimal(
                            filter_definition.min_qty.clone(),
                            operation,
                            "minQty",
                        )?)
                        .max_quantity(internal::parse_optional_decimal(
                            filter_definition.max_qty.clone(),
                            operation,
                            "maxQty",
                        )?)
                        .step_size(internal::parse_optional_decimal(
                            filter_definition.step_size.clone(),
                            operation,
                            "stepSize",
                        )?)
                        .build()
                        .map_err(|err| {
                            crate::error::invalid_field(operation, "LOT_SIZE", err.to_string())
                        })?,
                );
            }
            SymbolFilters::MarketLotSize(filter_definition) => {
                market_lot_size = Some(
                    LotSizeFilter::builder()
                        .min_quantity(internal::parse_optional_decimal(
                            filter_definition.min_qty.clone(),
                            operation,
                            "minQty",
                        )?)
                        .max_quantity(internal::parse_optional_decimal(
                            filter_definition.max_qty.clone(),
                            operation,
                            "maxQty",
                        )?)
                        .step_size(internal::parse_optional_decimal(
                            filter_definition.step_size.clone(),
                            operation,
                            "stepSize",
                        )?)
                        .build()
                        .map_err(|err| {
                            crate::error::invalid_field(
                                operation,
                                "MARKET_LOT_SIZE",
                                err.to_string(),
                            )
                        })?,
                );
            }
            SymbolFilters::MinNotional(filter_definition) => {
                min_notional = internal::parse_optional_decimal(
                    filter_definition.min_notional.clone(),
                    operation,
                    "minNotional",
                )?;
                apply_min_to_market = filter_definition.apply_to_market;
            }
            SymbolFilters::Notional(filter_definition) => {
                if let Some(value) = internal::parse_optional_decimal(
                    filter_definition.min_notional.clone(),
                    operation,
                    "minNotional",
                )? {
                    min_notional = Some(value);
                }
                max_notional = internal::parse_optional_decimal(
                    filter_definition.max_notional.clone(),
                    operation,
                    "maxNotional",
                )?;
                apply_min_to_market = filter_definition.apply_min_to_market;
                apply_max_to_market = filter_definition.apply_max_to_market;
            }
            _ => {}
        }
    }

    let notional = if min_notional.is_some()
        || max_notional.is_some()
        || apply_min_to_market.is_some()
        || apply_max_to_market.is_some()
    {
        Some(
            NotionalConstraints::builder()
                .min_notional(min_notional)
                .max_notional(max_notional)
                .apply_min_to_market(apply_min_to_market)
                .apply_max_to_market(apply_max_to_market)
                .build()
                .map_err(|err| {
                    crate::error::invalid_field(operation, "NOTIONAL", err.to_string())
                })?,
        )
    } else {
        None
    };

    let mut quantity_mode_support = vec![
        QuantityModeSupport::builder()
            .mode(MarketQuantityMode::Base)
            .order_types(allowed_order_types.clone())
            .sides([mkt_types::OrderSide::Buy, mkt_types::OrderSide::Sell])
            .build()
            .map_err(|err| {
                crate::error::invalid_field(operation, "quantity_mode_support", err.to_string())
            })?,
    ];

    if symbol_definition.quote_order_qty_market_allowed.unwrap_or(false) {
        quantity_mode_support.push(
            QuantityModeSupport::builder()
                .mode(MarketQuantityMode::Quote)
                .order_types([mkt_types::OrderType::Market])
                .sides([mkt_types::OrderSide::Buy])
                .build()
                .map_err(|err| {
                    crate::error::invalid_field(
                        operation,
                        "quantity_mode_support",
                        err.to_string(),
                    )
                })?,
        );
    }

    let trading_permissions = TradingPermissions::builder()
        .spot_order_entry_allowed(symbol_definition.is_spot_trading_allowed)
        .supported_order_types(allowed_order_types)
        .quantity_mode_support(quantity_mode_support)
        .build()
        .map_err(|err| {
            crate::error::invalid_field(operation, "trading_permissions", err.to_string())
        })?;

    let trading_constraints = TradingConstraints::builder()
        .price_filter(price_filter)
        .lot_size(lot_size)
        .market_lot_size(market_lot_size)
        .notional(notional)
        .build()
        .map_err(|err| {
            crate::error::invalid_field(operation, "trading_constraints", err.to_string())
        })?;

    MarketInfo::builder()
        .exchange_id(ExchangeId::from(KnownExchange::Binance))
        .symbol(Symbol::spot(symbol_definition.symbol.ok_or_else(|| {
            crate::error::missing_field(operation, "symbol")
        })?))
        .status(status)
        .base_asset(
            symbol_definition
                .base_asset
                .ok_or_else(|| crate::error::missing_field(operation, "baseAsset"))?,
        )
        .quote_asset(
            symbol_definition
                .quote_asset
                .ok_or_else(|| crate::error::missing_field(operation, "quoteAsset"))?,
        )
        .trading_permissions(trading_permissions)
        .trading_constraints(trading_constraints)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "market", err.to_string()))
}

pub(crate) fn last_prices_from_response(
    response: TickerPriceResponse,
    operation: &'static str,
) -> Result<Vec<LastPrice>> {
    let map_entry = |symbol: Option<String>, price: Option<String>| -> Result<LastPrice> {
        Ok(LastPrice::new(
            Symbol::spot(symbol.ok_or_else(|| crate::error::missing_field(operation, "symbol"))?),
            Decimal::from_str(
                price
                    .ok_or_else(|| crate::error::missing_field(operation, "price"))?
                    .as_str(),
            )
            .map_err(|err| crate::error::invalid_field(operation, "price", err.to_string()))?,
        ))
    };

    match response {
        TickerPriceResponse::TickerPriceResponse1(entry) => {
            Ok(vec![map_entry(entry.symbol, entry.price)?])
        }
        TickerPriceResponse::TickerPriceResponse2(entries) => entries
            .into_iter()
            .map(|entry| map_entry(entry.symbol, entry.price))
            .collect(),
        TickerPriceResponse::Other(other) => Err(crate::error::invalid_field(
            operation,
            "ticker_price",
            format!("unexpected response shape: {other}"),
        )),
    }
}

pub(crate) fn order_book_from_depth(
    symbol: &Symbol,
    response: binance_sdk::spot::rest_api::DepthResponse,
    operation: &'static str,
) -> Result<OrderBook> {
    let parse_levels =
        |levels: Option<Vec<Vec<String>>>, field: &'static str| -> Result<Vec<OrderBookLevel>> {
            levels
                .unwrap_or_default()
                .into_iter()
                .map(|level| {
                    if level.len() < 2 {
                        return Err(crate::error::invalid_field(
                            operation,
                            field,
                            "expected price/quantity level pair",
                        ));
                    }

                    Ok(OrderBookLevel::new(
                        Decimal::from_str(level[0].as_str()).map_err(|err| {
                            crate::error::invalid_field(operation, field, err.to_string())
                        })?,
                        Decimal::from_str(level[1].as_str()).map_err(|err| {
                            crate::error::invalid_field(operation, field, err.to_string())
                        })?,
                    ))
                })
                .collect()
        };

    OrderBook::builder()
        .symbol(symbol.clone())
        .bids(parse_levels(response.bids, "bids")?)
        .asks(parse_levels(response.asks, "asks")?)
        .last_update_id(response.last_update_id.map(|value| value.to_string()))
        .timestamp(None)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "order_book", err.to_string()))
}

pub(crate) fn trades_from_recent_response(
    symbol: &Symbol,
    response: Vec<HistoricalTradesResponseInner>,
    operation: &'static str,
) -> Result<Vec<Trade>> {
    response
        .into_iter()
        .map(|trade| {
            Trade::builder()
                .symbol(symbol.clone())
                .id(trade.id.map(|value| value.to_string()))
                .price(
                    Decimal::from_str(
                        trade
                            .price
                            .ok_or_else(|| crate::error::missing_field(operation, "price"))?
                            .as_str(),
                    )
                    .map_err(|err| {
                        crate::error::invalid_field(operation, "price", err.to_string())
                    })?,
                )
                .quantity(
                    Decimal::from_str(
                        trade
                            .qty
                            .ok_or_else(|| crate::error::missing_field(operation, "qty"))?
                            .as_str(),
                    )
                    .map_err(|err| {
                        crate::error::invalid_field(operation, "qty", err.to_string())
                    })?,
                )
                .side(match trade.is_buyer_maker {
                    Some(true) => TradeSide::Sell,
                    Some(false) => TradeSide::Buy,
                    None => return Err(crate::error::missing_field(operation, "isBuyerMaker")),
                })
                .timestamp(internal::parse_unix_millis_timestamp(
                    trade
                        .time
                        .ok_or_else(|| crate::error::missing_field(operation, "time"))?,
                    operation,
                    "time",
                )?)
                .build()
                .map_err(|err| crate::error::invalid_field(operation, "trade", err.to_string()))
        })
        .collect()
}

pub(crate) fn klines_from_rows(
    symbol: &Symbol,
    interval: KlineInterval,
    rows: Vec<Vec<KlinesItemInner>>,
    operation: &'static str,
) -> Result<Vec<Kline>> {
    let now = OffsetDateTime::now_utc();

    rows.into_iter()
        .map(|row| {
            if row.len() < 8 {
                return Err(crate::error::invalid_field(
                    operation,
                    "kline",
                    "expected at least 8 fields in Binance kline row",
                ));
            }

            let open_time_ms = match row.first() {
                Some(KlinesItemInner::Integer(value)) => *value,
                Some(other) => {
                    return Err(crate::error::invalid_field(
                        operation,
                        "openTime",
                        format!("expected integer, got {other:?}"),
                    ))
                }
                None => return Err(crate::error::missing_field(operation, "openTime")),
            };
            let close_time_ms = match row.get(6) {
                Some(KlinesItemInner::Integer(value)) => *value,
                Some(other) => {
                    return Err(crate::error::invalid_field(
                        operation,
                        "closeTime",
                        format!("expected integer, got {other:?}"),
                    ))
                }
                None => return Err(crate::error::missing_field(operation, "closeTime")),
            };
            let parse_decimal_field = |index: usize, field: &'static str| -> Result<Decimal> {
                match row.get(index) {
                    Some(KlinesItemInner::String(raw)) => {
                        Decimal::from_str(raw.as_str()).map_err(|err| {
                            crate::error::invalid_field(operation, field, err.to_string())
                        })
                    }
                    Some(KlinesItemInner::Integer(raw)) => {
                        Decimal::from_str(raw.to_string().as_str()).map_err(|err| {
                            crate::error::invalid_field(operation, field, err.to_string())
                        })
                    }
                    Some(other) => Err(crate::error::invalid_field(
                        operation,
                        field,
                        format!("expected decimal string, got {other:?}"),
                    )),
                    None => Err(crate::error::missing_field(operation, field)),
                }
            };

            let close_time =
                internal::parse_unix_millis_timestamp(close_time_ms, operation, "closeTime")?;

            Kline::builder()
                .symbol(symbol.clone())
                .interval(interval)
                .open_time(internal::parse_unix_millis_timestamp(
                    open_time_ms,
                    operation,
                    "openTime",
                )?)
                .close_time(close_time)
                .open(parse_decimal_field(1, "open")?)
                .high(parse_decimal_field(2, "high")?)
                .low(parse_decimal_field(3, "low")?)
                .close(parse_decimal_field(4, "close")?)
                .volume_base(parse_decimal_field(5, "volume")?)
                .volume_quote(Some(parse_decimal_field(7, "quoteAssetVolume")?))
                .closed(close_time < now)
                .build()
                .map_err(|err| crate::error::invalid_field(operation, "kline", err.to_string()))
        })
        .collect()
}
