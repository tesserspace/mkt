use mkt_core::Result;
use mkt_types::{
    ExchangeId, Kline, KlineInterval, KnownExchange, LastPrice, LotSizeFilter, MarketInfo,
    MarketQuantityMode, MarketStatus, NotionalConstraints, OrderBook, OrderBookLevel, OrderSide,
    OrderType, PriceFilter, QuantityModeSupport, Symbol, Trade, TradeSide, TradingConstraints,
    TradingPermissions,
};
use rust_decimal::Decimal;
use serde::Deserialize;

use super::internal;

#[derive(Debug, Deserialize)]
pub(crate) struct ExchangeInfoResponse {
    #[serde(default)]
    pub(crate) symbols: Option<Vec<ExchangeInfoSymbolResponse>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExchangeInfoSymbolResponse {
    symbol: Option<String>,
    status: Option<String>,
    base_asset: Option<String>,
    quote_asset: Option<String>,
    base_asset_precision: Option<i64>,
    quote_precision: Option<serde_json::Value>,
    quote_asset_precision: Option<i64>,
    base_size_precision: Option<String>,
    quote_amount_precision: Option<String>,
    quote_amount_precision_market: Option<String>,
    order_types: Option<Vec<String>>,
    is_spot_trading_allowed: Option<bool>,
    quote_order_qty_market_allowed: Option<bool>,
    filters: Option<Vec<ExchangeFilterResponse>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "filterType", rename_all = "SCREAMING_SNAKE_CASE")]
enum ExchangeFilterResponse {
    PriceFilter {
        min_price: Option<String>,
        max_price: Option<String>,
        tick_size: Option<String>,
    },
    LotSize {
        min_qty: Option<String>,
        max_qty: Option<String>,
        step_size: Option<String>,
    },
    MarketLotSize {
        min_qty: Option<String>,
        max_qty: Option<String>,
        step_size: Option<String>,
    },
    MinNotional {
        min_notional: Option<String>,
        apply_to_market: Option<bool>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum TickerPriceResponse {
    Single(TickerPriceEntryResponse),
    Multiple(Vec<TickerPriceEntryResponse>),
}

#[derive(Debug, Deserialize)]
pub(crate) struct TickerPriceEntryResponse {
    symbol: Option<String>,
    price: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OrderBookResponse {
    #[serde(default)]
    last_update_id: Option<serde_json::Value>,
    bids: Option<Vec<[String; 2]>>,
    asks: Option<Vec<[String; 2]>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TradeResponse {
    #[serde(default)]
    id: Option<serde_json::Value>,
    price: Option<String>,
    qty: Option<String>,
    time: Option<i64>,
    is_buyer_maker: Option<bool>,
}

pub(crate) fn markets_from_exchange_info_response(
    response: ExchangeInfoResponse,
    operation: &'static str,
) -> Result<Vec<MarketInfo>> {
    response
        .symbols
        .ok_or_else(|| crate::error::missing_field(operation, "symbols"))?
        .into_iter()
        .map(|market| market_info_from_response(market, operation))
        .collect()
}

pub(crate) fn last_prices_from_response(
    response: TickerPriceResponse,
    operation: &'static str,
) -> Result<Vec<LastPrice>> {
    match response {
        TickerPriceResponse::Single(entry) => Ok(vec![last_price_from_response(entry, operation)?]),
        TickerPriceResponse::Multiple(entries) => entries
            .into_iter()
            .map(|entry| last_price_from_response(entry, operation))
            .collect(),
    }
}

pub(crate) fn order_book_from_response(
    symbol: &Symbol,
    response: OrderBookResponse,
    operation: &'static str,
) -> Result<OrderBook> {
    OrderBook::builder()
        .symbol(symbol.clone())
        .bids(levels_from_rows(
            response
                .bids
                .ok_or_else(|| crate::error::missing_field(operation, "bids"))?,
            operation,
            "bids",
        )?)
        .asks(levels_from_rows(
            response
                .asks
                .ok_or_else(|| crate::error::missing_field(operation, "asks"))?,
            operation,
            "asks",
        )?)
        .last_update_id(optional_value_to_string(response.last_update_id))
        .timestamp(None)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "order_book", err.to_string()))
}

pub(crate) fn trades_from_response(
    symbol: &Symbol,
    response: Vec<TradeResponse>,
    operation: &'static str,
) -> Result<Vec<Trade>> {
    response
        .into_iter()
        .map(|trade| trade_from_response(symbol, trade, operation))
        .collect()
}

pub(crate) fn klines_from_rows(
    symbol: &Symbol,
    interval: KlineInterval,
    rows: Vec<Vec<serde_json::Value>>,
    operation: &'static str,
) -> Result<Vec<Kline>> {
    rows.into_iter()
        .map(|row| kline_from_row(symbol, interval, row, operation))
        .collect()
}

fn market_info_from_response(
    market: ExchangeInfoSymbolResponse,
    operation: &'static str,
) -> Result<MarketInfo> {
    let status = market_status_from_raw(
        market
            .status
            .as_deref()
            .ok_or_else(|| crate::error::missing_field(operation, "status"))?,
        operation,
    )?;

    let mut supported_order_types = Vec::new();
    for raw in market.order_types.unwrap_or_default() {
        let order_type = order_type_from_raw(&raw, operation)?;
        if !supported_order_types.contains(&order_type) {
            supported_order_types.push(order_type);
        }
    }

    let (
        price_filter,
        mut lot_size,
        mut market_lot_size,
        mut min_notional,
        mut apply_min_to_market,
    ) = filter_constraints(market.filters.unwrap_or_default(), operation)?;
    let base_size_precision =
        positive_decimal(market.base_size_precision, operation, "baseSizePrecision")?;
    if lot_size.is_none() {
        lot_size = base_size_precision
            .map(|value| lot_size_from_base_size_precision(value, operation))
            .transpose()?;
    }
    if market_lot_size.is_none() {
        market_lot_size = base_size_precision
            .map(|value| lot_size_from_base_size_precision(value, operation))
            .transpose()?;
    }
    if min_notional.is_none() {
        min_notional = positive_decimal(
            market
                .quote_amount_precision_market
                .or(market.quote_amount_precision),
            operation,
            "quoteAmountPrecision",
        )?;
        if min_notional.is_some() && apply_min_to_market.is_none() {
            apply_min_to_market = Some(true);
        }
    }
    let quantity_mode_support = quantity_mode_support(
        &supported_order_types,
        market.quote_order_qty_market_allowed,
        operation,
    )?;

    MarketInfo::builder()
        .exchange_id(ExchangeId::from(KnownExchange::Mexc))
        .symbol(Symbol::spot(market.symbol.ok_or_else(|| {
            crate::error::missing_field(operation, "symbol")
        })?))
        .status(status)
        .base_asset(
            market
                .base_asset
                .ok_or_else(|| crate::error::missing_field(operation, "baseAsset"))?,
        )
        .quote_asset(
            market
                .quote_asset
                .ok_or_else(|| crate::error::missing_field(operation, "quoteAsset"))?,
        )
        .base_asset_precision(market.base_asset_precision)
        .quote_precision(internal::parse_optional_i64(
            market.quote_precision.map(internal::value_to_string),
            operation,
            "quotePrecision",
        )?)
        .quote_asset_precision(market.quote_asset_precision)
        .trading_permissions(
            TradingPermissions::builder()
                .spot_order_entry_allowed(market.is_spot_trading_allowed)
                .supported_order_types(supported_order_types)
                .quantity_mode_support(quantity_mode_support)
                .build()
                .map_err(|err| {
                    crate::error::invalid_field(operation, "trading_permissions", err.to_string())
                })?,
        )
        .trading_constraints(
            TradingConstraints::builder()
                .price_filter(price_filter)
                .lot_size(lot_size)
                .market_lot_size(market_lot_size)
                .notional(
                    (min_notional.is_some() || apply_min_to_market.is_some())
                        .then(|| {
                            NotionalConstraints::builder()
                                .min_notional(min_notional)
                                .apply_min_to_market(apply_min_to_market)
                                .build()
                                .map_err(|err| {
                                    crate::error::invalid_field(
                                        operation,
                                        "notional_constraints",
                                        err.to_string(),
                                    )
                                })
                        })
                        .transpose()?,
                )
                .build()
                .map_err(|err| {
                    crate::error::invalid_field(operation, "trading_constraints", err.to_string())
                })?,
        )
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "market_info", err.to_string()))
}

pub(super) fn market_status_from_raw(raw: &str, operation: &'static str) -> Result<MarketStatus> {
    match raw {
        "1" | "ENABLED" => Ok(MarketStatus::Trading),
        "2" | "OFFLINE" | "PAUSE" => Ok(MarketStatus::Halted),
        "3" => Ok(MarketStatus::Halted),
        other => Err(crate::error::invalid_field(
            operation,
            "status",
            format!("unsupported MEXC spot symbol status `{other}`"),
        )),
    }
}

type FiltersResult = (
    Option<PriceFilter>,
    Option<LotSizeFilter>,
    Option<LotSizeFilter>,
    Option<rust_decimal::Decimal>,
    Option<bool>,
);

fn filter_constraints(
    filters: Vec<ExchangeFilterResponse>,
    operation: &'static str,
) -> Result<FiltersResult> {
    let mut price_filter = None;
    let mut lot_size = None;
    let mut market_lot_size = None;
    let mut min_notional = None;
    let mut apply_min_to_market = None;

    for filter in filters {
        match filter {
            ExchangeFilterResponse::PriceFilter {
                min_price,
                max_price,
                tick_size,
            } => {
                price_filter = Some(
                    PriceFilter::builder()
                        .min_price(internal::parse_optional_decimal(
                            min_price, operation, "minPrice",
                        )?)
                        .max_price(internal::parse_optional_decimal(
                            max_price, operation, "maxPrice",
                        )?)
                        .tick_size(internal::parse_optional_decimal(
                            tick_size, operation, "tickSize",
                        )?)
                        .build()
                        .map_err(|err| {
                            crate::error::invalid_field(operation, "PRICE_FILTER", err.to_string())
                        })?,
                );
            }
            ExchangeFilterResponse::LotSize {
                min_qty,
                max_qty,
                step_size,
            } => {
                lot_size = Some(lot_size_filter(
                    min_qty, max_qty, step_size, operation, "LOT_SIZE",
                )?);
            }
            ExchangeFilterResponse::MarketLotSize {
                min_qty,
                max_qty,
                step_size,
            } => {
                market_lot_size = Some(lot_size_filter(
                    min_qty,
                    max_qty,
                    step_size,
                    operation,
                    "MARKET_LOT_SIZE",
                )?);
            }
            ExchangeFilterResponse::MinNotional {
                min_notional: value,
                apply_to_market,
            } => {
                min_notional = internal::parse_optional_decimal(value, operation, "minNotional")?;
                apply_min_to_market = apply_to_market;
            }
            ExchangeFilterResponse::Other => {}
        }
    }

    Ok((
        price_filter,
        lot_size,
        market_lot_size,
        min_notional,
        apply_min_to_market,
    ))
}

fn lot_size_filter(
    min_qty: Option<String>,
    max_qty: Option<String>,
    step_size: Option<String>,
    operation: &'static str,
    field: &'static str,
) -> Result<LotSizeFilter> {
    LotSizeFilter::builder()
        .min_quantity(internal::parse_optional_decimal(
            min_qty, operation, "minQty",
        )?)
        .max_quantity(internal::parse_optional_decimal(
            max_qty, operation, "maxQty",
        )?)
        .step_size(internal::parse_optional_decimal(
            step_size, operation, "stepSize",
        )?)
        .build()
        .map_err(|err| crate::error::invalid_field(operation, field, err.to_string()))
}

fn lot_size_from_base_size_precision(
    base_size_precision: Decimal,
    operation: &'static str,
) -> Result<LotSizeFilter> {
    LotSizeFilter::builder()
        .min_quantity(Some(base_size_precision))
        .step_size(Some(base_size_precision))
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "baseSizePrecision", err.to_string()))
}

fn positive_decimal(
    raw: Option<String>,
    operation: &'static str,
    field: &'static str,
) -> Result<Option<Decimal>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let value = internal::parse_decimal(raw, operation, field)?;
    Ok((value > Decimal::ZERO).then_some(value))
}

fn quantity_mode_support(
    supported_order_types: &[OrderType],
    quote_market_allowed: Option<bool>,
    operation: &'static str,
) -> Result<Vec<QuantityModeSupport>> {
    let mut support = vec![QuantityModeSupport::builder()
        .mode(MarketQuantityMode::Base)
        .order_types(supported_order_types.to_owned())
        .sides([OrderSide::Buy, OrderSide::Sell])
        .build()
        .map_err(|err| {
            crate::error::invalid_field(operation, "quantity_mode_support", err.to_string())
        })?];

    if quote_market_allowed.unwrap_or(false) {
        support.push(
            QuantityModeSupport::builder()
                .mode(MarketQuantityMode::Quote)
                .order_types([OrderType::Market])
                .sides([OrderSide::Buy])
                .build()
                .map_err(|err| {
                    crate::error::invalid_field(operation, "quantity_mode_support", err.to_string())
                })?,
        );
    }

    Ok(support)
}

fn last_price_from_response(
    entry: TickerPriceEntryResponse,
    operation: &'static str,
) -> Result<LastPrice> {
    Ok(LastPrice::new(
        Symbol::spot(
            entry
                .symbol
                .ok_or_else(|| crate::error::missing_field(operation, "symbol"))?,
        ),
        internal::parse_required_decimal(entry.price, operation, "price")?,
    ))
}

fn trade_from_response(
    symbol: &Symbol,
    trade: TradeResponse,
    operation: &'static str,
) -> Result<Trade> {
    let mut builder = Trade::builder()
        .symbol(symbol.clone())
        .price(internal::parse_required_decimal(
            trade.price,
            operation,
            "price",
        )?)
        .quantity(internal::parse_required_decimal(
            trade.qty, operation, "qty",
        )?)
        .side(match trade.is_buyer_maker {
            Some(true) => TradeSide::Sell,
            Some(false) => TradeSide::Buy,
            None => return Err(crate::error::missing_field(operation, "isBuyerMaker")),
        })
        .timestamp(internal::parse_unix_millis_timestamp(
            internal::parse_required_i64(trade.time, operation, "time")?,
            operation,
            "time",
        )?);
    if let Some(id) = trade.id {
        builder = builder.id(internal::value_to_string(id));
    }
    builder
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "trade", err.to_string()))
}

fn kline_from_row(
    symbol: &Symbol,
    interval: KlineInterval,
    row: Vec<serde_json::Value>,
    operation: &'static str,
) -> Result<Kline> {
    if row.len() < 8 {
        return Err(crate::error::invalid_field(
            operation,
            "kline",
            "expected at least 8 fields in MEXC kline row",
        ));
    }

    let close_time = internal::parse_value_timestamp(&row[6], operation, "closeTime")?;
    Kline::builder()
        .symbol(symbol.clone())
        .interval(interval)
        .open_time(internal::parse_value_timestamp(
            &row[0], operation, "openTime",
        )?)
        .open(internal::parse_value_decimal(&row[1], operation, "open")?)
        .high(internal::parse_value_decimal(&row[2], operation, "high")?)
        .low(internal::parse_value_decimal(&row[3], operation, "low")?)
        .close(internal::parse_value_decimal(&row[4], operation, "close")?)
        .volume_base(internal::parse_value_decimal(&row[5], operation, "volume")?)
        .close_time(close_time)
        .volume_quote(Some(internal::parse_value_decimal(
            &row[7],
            operation,
            "quoteVolume",
        )?))
        .closed(internal::closed_from_close_time(close_time))
        .build()
        .map_err(|err| crate::error::invalid_field(operation, "kline", err.to_string()))
}

fn levels_from_rows(
    rows: Vec<[String; 2]>,
    operation: &'static str,
    field: &'static str,
) -> Result<Vec<OrderBookLevel>> {
    rows.into_iter()
        .map(|row| {
            Ok(OrderBookLevel::new(
                internal::parse_decimal(row[0].clone(), operation, field)?,
                internal::parse_decimal(row[1].clone(), operation, field)?,
            ))
        })
        .collect()
}

fn order_type_from_raw(raw: &str, operation: &'static str) -> Result<OrderType> {
    match raw {
        "LIMIT" => Ok(OrderType::Limit),
        "MARKET" => Ok(OrderType::Market),
        "LIMIT_MAKER" => Ok(OrderType::PostOnly),
        other => Err(crate::error::invalid_field(
            operation,
            "orderTypes",
            format!("unsupported MEXC order type `{other}`"),
        )),
    }
}

fn optional_value_to_string(value: Option<serde_json::Value>) -> Option<String> {
    value.map(internal::value_to_string)
}

#[cfg(test)]
mod tests;
