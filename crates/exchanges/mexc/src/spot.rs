use std::sync::Arc;

use async_trait::async_trait;
use mkt_core::{Account, Error, Result, SpotTrading};
use mkt_exchange_common as common;
use mkt_types::{
    Balance, ExchangeId, Fill, KnownExchange, Order, OrderKey, SpotCancelOrderRequest,
    SpotOrderQuery, SpotOrderRequest, Symbol,
};
use time::OffsetDateTime;

use crate::{convert, error, rest::query_pair, rest::MexcRestClient, MexcInner};

const BALANCES_OPERATION: &str = "account.balances";
const CANCEL_ORDER_OPERATION: &str = "spot.cancel_order";
const GET_ORDER_OPERATION: &str = "spot.get_order";
const OPEN_ORDERS_OPERATION: &str = "spot.open_orders";
const PLACE_ORDER_OPERATION: &str = "spot.place_order";
const SPOT_FILLS_OPERATION: &str = "spot.my_trades";

pub(crate) struct MexcSpotTrading {
    rest: MexcRestClient,
    has_credentials: bool,
}

impl MexcSpotTrading {
    pub(crate) fn new(inner: Arc<MexcInner>) -> Self {
        Self {
            has_credentials: inner.config.credentials.is_some(),
            rest: MexcRestClient::new(inner),
        }
    }

    fn ensure_credentials(&self) -> Result<()> {
        ensure_credentials(self.has_credentials)
    }

    async fn open_orders_for_symbol(&self, symbol: String) -> Result<Vec<Order>> {
        let response: Vec<convert::OpenOrderResponse> = self
            .rest
            .get_signed(
                OPEN_ORDERS_OPERATION,
                "api/v3/openOrders",
                vec![query_pair("symbol", symbol)],
            )
            .await?;
        response
            .into_iter()
            .map(|order| convert::order_from_snapshot(order.into(), OPEN_ORDERS_OPERATION))
            .collect()
    }
}

#[async_trait]
impl SpotTrading for MexcSpotTrading {
    async fn place_spot_order(&self, request: SpotOrderRequest) -> Result<Order> {
        self.ensure_credentials()?;

        let query = convert::build_new_order_query(&request, PLACE_ORDER_OPERATION)?;
        let response: convert::NewOrderResponse = self
            .rest
            .post_signed(PLACE_ORDER_OPERATION, "api/v3/order", query)
            .await?;
        let order = convert::order_from_snapshot(response.clone().into(), PLACE_ORDER_OPERATION);
        match order {
            Ok(order) => Ok(order),
            Err(_) => {
                let key = order_key_from_place_response(response, PLACE_ORDER_OPERATION)?;
                self.spot_order(SpotOrderQuery::new(request.symbol, key))
                    .await
            }
        }
    }

    async fn cancel_spot_order(&self, request: SpotCancelOrderRequest) -> Result<Order> {
        self.ensure_credentials()?;

        let symbol = convert::require_spot_symbol(&request.symbol, CANCEL_ORDER_OPERATION)?;
        let (order_id, orig_client_order_id) =
            convert::lookup_order_key(&request.key, CANCEL_ORDER_OPERATION)?;
        let mut query = vec![query_pair("symbol", symbol)];
        if let Some(order_id) = order_id {
            query.push(query_pair("orderId", order_id));
        }
        if let Some(client_order_id) = orig_client_order_id {
            query.push(query_pair("origClientOrderId", client_order_id));
        }
        let response: convert::DeleteOrderResponse = self
            .rest
            .delete_signed(CANCEL_ORDER_OPERATION, "api/v3/order", query)
            .await?;
        convert::order_from_snapshot(response.into(), CANCEL_ORDER_OPERATION)
    }

    async fn spot_order(&self, query: SpotOrderQuery) -> Result<Order> {
        self.ensure_credentials()?;

        let symbol = convert::require_spot_symbol(&query.symbol, GET_ORDER_OPERATION)?;
        let (order_id, orig_client_order_id) =
            convert::lookup_order_key(&query.key, GET_ORDER_OPERATION)?;
        let mut params = vec![query_pair("symbol", symbol)];
        if let Some(order_id) = order_id {
            params.push(query_pair("orderId", order_id));
        }
        if let Some(client_order_id) = orig_client_order_id {
            params.push(query_pair("origClientOrderId", client_order_id));
        }
        let response: convert::GetOrderResponse = self
            .rest
            .get_signed(GET_ORDER_OPERATION, "api/v3/order", params)
            .await?;
        convert::order_from_snapshot(response.into(), GET_ORDER_OPERATION)
    }

    async fn open_spot_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>> {
        self.ensure_credentials()?;

        let symbol = symbol.ok_or_else(|| {
            error::invalid_field(
                OPEN_ORDERS_OPERATION,
                "symbol",
                "MEXC spot GET /api/v3/openOrders requires a single symbol and does not support querying all symbols at once",
            )
        })?;
        self.open_orders_for_symbol(convert::require_spot_symbol(symbol, OPEN_ORDERS_OPERATION)?)
            .await
    }

    async fn spot_fills(&self, query: SpotOrderQuery) -> Result<Vec<Fill>> {
        self.ensure_credentials()?;

        let symbol = query.symbol;
        let symbol_name = convert::require_spot_symbol(&symbol, SPOT_FILLS_OPERATION)?;
        let (order_id, order_id_filter) = match query.key {
            OrderKey::Exchange(order_id) => fill_order_id_lookup(&order_id.0),
            OrderKey::Client(client_order_id) => {
                let order = self
                    .spot_order(SpotOrderQuery::new(
                        symbol.clone(),
                        OrderKey::Client(client_order_id),
                    ))
                    .await?;
                fill_order_id_lookup(&order.id.0)
            }
            _ => {
                return Err(error::invalid_field(
                    SPOT_FILLS_OPERATION,
                    "key",
                    "unsupported MEXC spot order key",
                ))
            }
        };
        let mut params = vec![query_pair("symbol", symbol_name)];
        if let Some(order_id) = order_id {
            params.push(query_pair("orderId", order_id));
        }
        let response: Vec<convert::MyTradeResponse> = self
            .rest
            .get_signed(SPOT_FILLS_OPERATION, "api/v3/myTrades", params)
            .await?;

        response
            .into_iter()
            .filter(|trade| trade_matches_order_id_filter(trade, order_id_filter.as_deref()))
            .map(|trade| convert::fill_from_trade(&symbol, trade, SPOT_FILLS_OPERATION))
            .collect()
    }
}

pub(crate) struct MexcAccount {
    rest: MexcRestClient,
    has_credentials: bool,
}

impl MexcAccount {
    pub(crate) fn new(inner: Arc<MexcInner>) -> Self {
        Self {
            has_credentials: inner.config.credentials.is_some(),
            rest: MexcRestClient::new(inner),
        }
    }

    fn ensure_credentials(&self) -> Result<()> {
        ensure_credentials(self.has_credentials)
    }
}

#[async_trait]
impl Account for MexcAccount {
    async fn balances(&self) -> Result<Vec<Balance>> {
        self.ensure_credentials()?;

        let response: convert::AccountResponse = self
            .rest
            .get_signed(BALANCES_OPERATION, "api/v3/account", Vec::new())
            .await?;
        let received_at = OffsetDateTime::now_utc();
        response
            .balances
            .into_iter()
            .map(|balance| {
                convert::balance_from_account_balance(
                    balance,
                    response.update_time,
                    received_at,
                    BALANCES_OPERATION,
                )
            })
            .collect()
    }
}

fn ensure_credentials(has_credentials: bool) -> Result<()> {
    if !has_credentials {
        return Err(Error::missing_credentials(ExchangeId::from(
            KnownExchange::Mexc,
        )));
    }
    Ok(())
}

fn order_key_from_place_response(
    response: convert::NewOrderResponse,
    operation: &'static str,
) -> Result<OrderKey> {
    if let Some(order_id) = response.order_id {
        let order_id = serde_json::Value::to_string(&order_id)
            .trim_matches('"')
            .to_owned();
        if convert::parse_exchange_order_id(&order_id, operation).is_ok() {
            return Ok(OrderKey::Exchange(mkt_types::OrderId::new(order_id)));
        }
        if let Some(client_order_id) = response.client_order_id {
            return Ok(OrderKey::Client(mkt_types::ClientOrderId::new(
                client_order_id,
            )));
        }
        return Ok(OrderKey::Exchange(mkt_types::OrderId::new(order_id)));
    }
    if let Some(client_order_id) = response.client_order_id {
        return Ok(OrderKey::Client(mkt_types::ClientOrderId::new(
            client_order_id,
        )));
    }
    Err(error::missing_field(operation, "orderId/clientOrderId"))
}

fn fill_order_id_lookup(raw: &str) -> (Option<i64>, Option<String>) {
    match convert::parse_exchange_order_id(raw, SPOT_FILLS_OPERATION) {
        Ok(order_id) => (Some(order_id), None),
        Err(_) => (None, Some(raw.to_owned())),
    }
}

fn trade_matches_order_id_filter(
    trade: &convert::MyTradeResponse,
    order_id_filter: Option<&str>,
) -> bool {
    let Some(expected) = order_id_filter else {
        return true;
    };
    trade
        .order_id
        .as_ref()
        .map(common::value_to_string)
        .is_some_and(|order_id| order_id == expected)
}

#[cfg(test)]
mod tests {
    use mkt_types::{
        OrderKey, OrderQuantity, OrderSide, OrderType, SpotOrderRequest, Symbol, TimeInForce,
    };
    use rust_decimal::Decimal;

    use super::{fill_order_id_lookup, order_key_from_place_response};

    #[test]
    fn place_order_builds_signed_query_without_network() {
        let query = crate::convert::build_new_order_query(
            &SpotOrderRequest::builder()
                .symbol(Symbol::spot("BTCUSDT"))
                .side(OrderSide::Buy)
                .order_type(OrderType::Limit)
                .quantity(OrderQuantity::Base(Decimal::new(1, 0)))
                .price(Some(Decimal::new(43000, 0)))
                .time_in_force(Some(TimeInForce::Gtc))
                .client_order_id(Some(mkt_types::ClientOrderId::new("c-1")))
                .build()
                .expect("request should build"),
            "spot.place_order",
        )
        .expect("request should convert");

        assert!(query.iter().any(|(k, v)| *k == "symbol" && v == "BTCUSDT"));
        assert!(query.iter().any(|(k, v)| *k == "type" && v == "LIMIT"));
        assert!(query.iter().any(|(k, v)| *k == "side" && v == "BUY"));
    }

    #[test]
    fn place_response_uses_client_key_when_string_order_id_cannot_be_queried() {
        let response: crate::convert::NewOrderResponse =
            serde_json::from_value(serde_json::json!({
                "orderId": "abc-123",
                "clientOrderId": "client-1"
            }))
            .expect("string orderId fixture should deserialize");

        let key = order_key_from_place_response(response, "spot.place_order")
            .expect("client key should be used when MEXC orderId is not query-compatible");
        assert!(
            matches!(key, OrderKey::Client(client_order_id) if client_order_id.0 == "client-1")
        );
    }

    #[test]
    fn place_response_preserves_string_order_id_without_client_fallback() {
        let response: crate::convert::NewOrderResponse =
            serde_json::from_value(serde_json::json!({
                "orderId": "abc-123"
            }))
            .expect("string orderId fixture should deserialize");

        let key = order_key_from_place_response(response, "spot.place_order")
            .expect("string orderId without client id should map to exchange order key");
        assert!(matches!(key, OrderKey::Exchange(order_id) if order_id.0 == "abc-123"));
    }

    #[test]
    fn place_response_keeps_numeric_order_id_compatible() {
        let response: crate::convert::NewOrderResponse =
            serde_json::from_value(serde_json::json!({
                "orderId": 42,
                "clientOrderId": "client-1"
            }))
            .expect("numeric orderId fixture should deserialize");

        let key = order_key_from_place_response(response, "spot.place_order")
            .expect("numeric orderId should map to exchange order key");
        assert!(matches!(key, OrderKey::Exchange(order_id) if order_id.0 == "42"));
    }

    #[test]
    fn fill_lookup_filters_locally_for_non_numeric_mexc_order_ids() {
        assert_eq!(fill_order_id_lookup("42"), (Some(42), None));
        assert_eq!(
            fill_order_id_lookup("C02__685820380593467393042"),
            (None, Some("C02__685820380593467393042".to_owned()))
        );
    }

    #[test]
    fn open_spot_orders_without_symbol_error_message_is_explicit() {
        let err = crate::error::invalid_field(
            "spot.open_orders",
            "symbol",
            "MEXC spot GET /api/v3/openOrders requires a single symbol and does not support querying all symbols at once",
        );
        assert!(err.to_string().contains("requires a single symbol"));
    }
}
