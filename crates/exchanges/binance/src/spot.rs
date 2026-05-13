use std::sync::Arc;

use async_trait::async_trait;
use binance_sdk::spot::rest_api::{
    DeleteOrderParams, GetAccountParams, GetOpenOrdersParams, GetOrderParams, MyTradesParams,
};

use mkt_core::{Account, Error, Result, SpotTrading};
use mkt_types::{ExchangeId, KnownExchange};
use mkt_types::{
    Fill, Order, OrderKey, SpotCancelOrderRequest, SpotOrderQuery, SpotOrderRequest, Symbol,
};

use crate::{convert, error, BinanceInner};

const BALANCES_OPERATION: &str = "account.balances";
const CANCEL_ORDER_OPERATION: &str = "spot.cancel_order";
const GET_ORDER_OPERATION: &str = "spot.get_order";
const OPEN_ORDERS_OPERATION: &str = "spot.open_orders";
const PLACE_ORDER_OPERATION: &str = "spot.place_order";
const SPOT_FILLS_OPERATION: &str = "spot.my_trades";

pub(crate) struct BinanceSpotTrading {
    inner: Arc<BinanceInner>,
}

impl BinanceSpotTrading {
    pub(crate) fn new(inner: Arc<BinanceInner>) -> Self {
        Self { inner }
    }

    fn ensure_credentials(&self) -> Result<()> {
        if self.inner.config.credentials.is_none() {
            return Err(Error::missing_credentials(ExchangeId::from(
                KnownExchange::Binance,
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl SpotTrading for BinanceSpotTrading {
    async fn place_spot_order(&self, request: SpotOrderRequest) -> Result<Order> {
        self.ensure_credentials()?;

        let params = convert::build_new_order_params(&request, PLACE_ORDER_OPERATION)?;
        let response = self
            .inner
            .spot_rest
            .new_order(params)
            .await
            .map_err(|err| error::map_request_error(PLACE_ORDER_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(PLACE_ORDER_OPERATION, err))?;

        convert::order_from_snapshot(data.into(), PLACE_ORDER_OPERATION)
    }

    async fn cancel_spot_order(&self, request: SpotCancelOrderRequest) -> Result<Order> {
        self.ensure_credentials()?;

        let symbol = convert::require_spot_symbol(&request.symbol, CANCEL_ORDER_OPERATION)?;
        let (order_id, orig_client_order_id) =
            convert::lookup_order_key(&request.key, CANCEL_ORDER_OPERATION)?;
        let mut builder = DeleteOrderParams::builder(symbol);
        if let Some(order_id) = order_id {
            builder = builder.order_id(order_id);
        }
        if let Some(orig_client_order_id) = orig_client_order_id {
            builder = builder.orig_client_order_id(orig_client_order_id);
        }
        let params = builder
            .build()
            .map_err(|err| error::adapter_error(CANCEL_ORDER_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .delete_order(params)
            .await
            .map_err(|err| error::map_request_error(CANCEL_ORDER_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(CANCEL_ORDER_OPERATION, err))?;

        convert::order_from_snapshot(data.into(), CANCEL_ORDER_OPERATION)
    }

    async fn spot_order(&self, query: SpotOrderQuery) -> Result<Order> {
        self.ensure_credentials()?;

        let symbol = convert::require_spot_symbol(&query.symbol, GET_ORDER_OPERATION)?;
        let (order_id, orig_client_order_id) =
            convert::lookup_order_key(&query.key, GET_ORDER_OPERATION)?;
        let mut builder = GetOrderParams::builder(symbol);
        if let Some(order_id) = order_id {
            builder = builder.order_id(order_id);
        }
        if let Some(orig_client_order_id) = orig_client_order_id {
            builder = builder.orig_client_order_id(orig_client_order_id);
        }
        let params = builder
            .build()
            .map_err(|err| error::adapter_error(GET_ORDER_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .get_order(params)
            .await
            .map_err(|err| error::map_request_error(GET_ORDER_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(GET_ORDER_OPERATION, err))?;

        convert::order_from_snapshot(data.into(), GET_ORDER_OPERATION)
    }

    async fn open_spot_orders(&self, symbol: Option<&Symbol>) -> Result<Vec<Order>> {
        self.ensure_credentials()?;

        let mut builder = GetOpenOrdersParams::builder();
        if let Some(symbol) = symbol {
            builder = builder.symbol(convert::require_spot_symbol(symbol, OPEN_ORDERS_OPERATION)?);
        }
        let params = builder
            .build()
            .map_err(|err| error::adapter_error(OPEN_ORDERS_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .get_open_orders(params)
            .await
            .map_err(|err| error::map_request_error(OPEN_ORDERS_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(OPEN_ORDERS_OPERATION, err))?;

        data.into_iter()
            .map(|order| convert::order_from_snapshot(order.into(), OPEN_ORDERS_OPERATION))
            .collect()
    }

    async fn spot_fills(&self, query: SpotOrderQuery) -> Result<Vec<Fill>> {
        self.ensure_credentials()?;

        let symbol = query.symbol;
        let key = query.key;
        let symbol_name = convert::require_spot_symbol(&symbol, SPOT_FILLS_OPERATION)?;
        let order_id = match key {
            OrderKey::Exchange(order_id) => Some(convert::parse_exchange_order_id(
                order_id.0.as_str(),
                SPOT_FILLS_OPERATION,
            )?),
            OrderKey::Client(client_order_id) => {
                let order = self
                    .spot_order(SpotOrderQuery::new(
                        symbol,
                        OrderKey::Client(client_order_id),
                    ))
                    .await?;
                Some(convert::parse_exchange_order_id(
                    order.id.0.as_str(),
                    SPOT_FILLS_OPERATION,
                )?)
            }
            _ => {
                return Err(error::invalid_field(
                    SPOT_FILLS_OPERATION,
                    "key",
                    "unsupported Binance spot order key",
                ))
            }
        };
        let mut builder = MyTradesParams::builder(symbol_name);
        if let Some(order_id) = order_id {
            builder = builder.order_id(order_id);
        }
        let params = builder
            .build()
            .map_err(|err| error::adapter_error(SPOT_FILLS_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .my_trades(params)
            .await
            .map_err(|err| error::map_request_error(SPOT_FILLS_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(SPOT_FILLS_OPERATION, err))?;

        data.into_iter()
            .map(|fill| convert::fill_from_trade(fill, SPOT_FILLS_OPERATION))
            .collect()
    }
}

pub(crate) struct BinanceAccount {
    inner: Arc<BinanceInner>,
}

impl BinanceAccount {
    pub(crate) fn new(inner: Arc<BinanceInner>) -> Self {
        Self { inner }
    }

    fn ensure_credentials(&self) -> Result<()> {
        if self.inner.config.credentials.is_none() {
            return Err(Error::missing_credentials(ExchangeId::from(
                KnownExchange::Binance,
            )));
        }
        Ok(())
    }
}

#[async_trait]
impl Account for BinanceAccount {
    async fn balances(&self) -> Result<Vec<mkt_types::Balance>> {
        self.ensure_credentials()?;

        let params = GetAccountParams::builder()
            .omit_zero_balances(true)
            .build()
            .map_err(|err| error::adapter_error(BALANCES_OPERATION, err.to_string()))?;
        let response = self
            .inner
            .spot_rest
            .get_account(params)
            .await
            .map_err(|err| error::map_request_error(BALANCES_OPERATION, err))?;
        let data = response
            .data()
            .await
            .map_err(|err| error::map_connector_error(BALANCES_OPERATION, err))?;
        let update_time = data.update_time.ok_or_else(|| {
            error::invalid_field(BALANCES_OPERATION, "updateTime", "missing value")
        })?;

        data.balances
            .unwrap_or_default()
            .into_iter()
            .map(|balance| {
                convert::balance_from_account_balance(balance, update_time, BALANCES_OPERATION)
            })
            .collect()
    }
}
