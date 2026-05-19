use std::str::FromStr;

use mkt_core::Result;
use mkt_types::{Kline, KlineInterval, Symbol};
use rust_decimal::Decimal;
use time::OffsetDateTime;

use crate::{error, protobuf::PublicSpotKlineV3Api};

pub(crate) fn stream_interval(
    interval: KlineInterval,
    operation: &'static str,
) -> Result<&'static str> {
    match interval {
        KlineInterval::Minute(1) => Ok("Min1"),
        KlineInterval::Minute(5) => Ok("Min5"),
        KlineInterval::Minute(15) => Ok("Min15"),
        KlineInterval::Minute(30) => Ok("Min30"),
        KlineInterval::Hour(1) => Ok("Min60"),
        KlineInterval::Hour(4) => Ok("Hour4"),
        KlineInterval::Hour(8) => Ok("Hour8"),
        KlineInterval::Day(1) => Ok("Day1"),
        KlineInterval::Week(1) => Ok("Week1"),
        KlineInterval::Month(1) => Ok("Month1"),
        _ => Err(error::invalid_field(
            operation,
            "interval",
            "unsupported MEXC spot websocket kline interval",
        )),
    }
}

pub(crate) fn kline_from_proto(
    symbol: Symbol,
    interval: KlineInterval,
    payload: PublicSpotKlineV3Api,
    operation: &'static str,
) -> Result<Kline> {
    if !payload.interval.is_empty() {
        let expected = stream_interval(interval, operation)?;
        if payload.interval != expected {
            return Err(error::invalid_field(
                operation,
                "interval",
                format!(
                    "MEXC kline payload interval `{}` did not match subscription `{expected}`",
                    payload.interval
                ),
            ));
        }
    }

    Kline::builder()
        .symbol(symbol)
        .interval(interval)
        .open_time(parse_unix_seconds_timestamp(
            payload.window_start,
            operation,
            "windowStart",
        )?)
        .close_time(parse_unix_seconds_timestamp(
            payload.window_end,
            operation,
            "windowEnd",
        )?)
        .open(parse_decimal(
            payload.opening_price,
            operation,
            "openingPrice",
        )?)
        .high(parse_decimal(
            payload.highest_price,
            operation,
            "highestPrice",
        )?)
        .low(parse_decimal(
            payload.lowest_price,
            operation,
            "lowestPrice",
        )?)
        .close(parse_decimal(
            payload.closing_price,
            operation,
            "closingPrice",
        )?)
        .volume_base(parse_decimal(payload.volume, operation, "volume")?)
        .volume_quote(Some(parse_decimal(payload.amount, operation, "amount")?))
        .closed(is_closed(payload.window_end))
        .build()
        .map_err(|err| error::invalid_field(operation, "kline", err.to_string()))
}

fn parse_decimal(raw: String, operation: &'static str, field: &'static str) -> Result<Decimal> {
    Decimal::from_str(raw.as_str())
        .map_err(|err| error::invalid_field(operation, field, err.to_string()))
}

fn parse_unix_seconds_timestamp(
    timestamp_seconds: i64,
    operation: &'static str,
    field: &'static str,
) -> Result<OffsetDateTime> {
    if timestamp_seconds < 0 {
        return Err(error::invalid_field(
            operation,
            field,
            "invalid Unix second timestamp",
        ));
    }

    OffsetDateTime::from_unix_timestamp(timestamp_seconds)
        .map_err(|_| error::invalid_field(operation, field, "invalid Unix second timestamp"))
}

fn is_closed(window_end: i64) -> bool {
    match OffsetDateTime::from_unix_timestamp(window_end) {
        Ok(close_time) => close_time <= OffsetDateTime::now_utc(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::kline_from_proto;
    use crate::protobuf::PublicSpotKlineV3Api;
    use mkt_types::{KlineInterval, Symbol};
    use time::{Duration, OffsetDateTime};

    const OPERATION: &str = "stream.kline.test";

    #[test]
    fn stream_kline_closed_is_inferred_from_window_end() {
        let past = kline_from_proto(
            Symbol::spot("BTCUSDT"),
            KlineInterval::Minute(1),
            kline_payload(OffsetDateTime::now_utc() - Duration::minutes(1)),
            OPERATION,
        )
        .expect("past kline should convert");
        assert!(past.closed);

        let future = kline_from_proto(
            Symbol::spot("BTCUSDT"),
            KlineInterval::Minute(1),
            kline_payload(OffsetDateTime::now_utc() + Duration::minutes(1)),
            OPERATION,
        )
        .expect("future kline should convert");
        assert!(!future.closed);
    }

    fn kline_payload(window_end: OffsetDateTime) -> PublicSpotKlineV3Api {
        PublicSpotKlineV3Api {
            interval: "Min1".to_owned(),
            window_start: 1_700_000_000,
            window_end: window_end.unix_timestamp(),
            opening_price: "1.0".to_owned(),
            closing_price: "2.0".to_owned(),
            highest_price: "3.0".to_owned(),
            lowest_price: "0.5".to_owned(),
            volume: "10.0".to_owned(),
            amount: "20.0".to_owned(),
        }
    }
}
