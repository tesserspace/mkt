#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PushDataV3ApiWrapper {
    #[prost(string, tag = "1")]
    pub channel: String,
    #[prost(
        oneof = "push_data_v3_api_wrapper::Body",
        tags = "301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315"
    )]
    pub body: Option<push_data_v3_api_wrapper::Body>,
    #[prost(string, optional, tag = "3")]
    pub symbol: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub symbol_id: Option<String>,
    #[prost(int64, optional, tag = "5")]
    pub create_time: Option<i64>,
    #[prost(int64, optional, tag = "6")]
    pub send_time: Option<i64>,
}

pub(crate) mod push_data_v3_api_wrapper {
    // RATIONALE: This enum mirrors MEXC's official protobuf oneof tag layout.
    // Boxing variants would diverge from prost's generated shape for this schema,
    // so suppress the size lint at the schema-boundary only.
    #[allow(clippy::large_enum_variant)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub(crate) enum Body {
        #[prost(message, tag = "301")]
        PublicDeals(super::PublicDealsV3Api),
        #[prost(message, tag = "302")]
        PublicIncreaseDepths(super::PublicIncreaseDepthsV3Api),
        #[prost(message, tag = "303")]
        PublicLimitDepths(super::PublicLimitDepthsV3Api),
        #[prost(message, tag = "304")]
        PrivateOrders(super::PrivateOrdersV3Api),
        #[prost(message, tag = "305")]
        PublicBookTicker(super::PublicBookTickerV3Api),
        #[prost(message, tag = "306")]
        PrivateDeals(super::PrivateDealsV3Api),
        #[prost(message, tag = "307")]
        PrivateAccount(super::PrivateAccountV3Api),
        #[prost(message, tag = "308")]
        PublicSpotKline(super::PublicSpotKlineV3Api),
        #[prost(message, tag = "309")]
        PublicMiniTicker(super::PublicMiniTickerV3Api),
        #[prost(message, tag = "310")]
        PublicMiniTickers(super::PublicMiniTickersV3Api),
        #[prost(message, tag = "311")]
        PublicBookTickerBatch(super::PublicBookTickerBatchV3Api),
        #[prost(message, tag = "312")]
        PublicIncreaseDepthsBatch(super::PublicIncreaseDepthsBatchV3Api),
        #[prost(message, tag = "313")]
        PublicAggreDepths(super::PublicAggreDepthsV3Api),
        #[prost(message, tag = "314")]
        PublicAggreDeals(super::PublicAggreDealsV3Api),
        #[prost(message, tag = "315")]
        PublicAggreBookTicker(super::PublicAggreBookTickerV3Api),
    }
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicDealsV3Api {
    #[prost(message, repeated, tag = "1")]
    pub deals: Vec<PublicDealsV3ApiItem>,
    #[prost(string, tag = "2")]
    pub event_type: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicDealsV3ApiItem {
    #[prost(string, tag = "1")]
    pub price: String,
    #[prost(string, tag = "2")]
    pub quantity: String,
    #[prost(int32, tag = "3")]
    pub trade_type: i32,
    #[prost(int64, tag = "4")]
    pub time: i64,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicIncreaseDepthsV3Api {
    #[prost(message, repeated, tag = "1")]
    pub asks: Vec<PublicIncreaseDepthV3ApiItem>,
    #[prost(message, repeated, tag = "2")]
    pub bids: Vec<PublicIncreaseDepthV3ApiItem>,
    #[prost(string, tag = "3")]
    pub event_type: String,
    #[prost(string, tag = "4")]
    pub version: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicIncreaseDepthV3ApiItem {
    #[prost(string, tag = "1")]
    pub price: String,
    #[prost(string, tag = "2")]
    pub quantity: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicLimitDepthsV3Api {
    #[prost(message, repeated, tag = "1")]
    pub asks: Vec<PublicLimitDepthV3ApiItem>,
    #[prost(message, repeated, tag = "2")]
    pub bids: Vec<PublicLimitDepthV3ApiItem>,
    #[prost(string, tag = "3")]
    pub event_type: String,
    #[prost(string, tag = "4")]
    pub version: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicLimitDepthV3ApiItem {
    #[prost(string, tag = "1")]
    pub price: String,
    #[prost(string, tag = "2")]
    pub quantity: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PrivateOrdersV3Api {
    #[prost(string, tag = "1")]
    pub id: String,
    #[prost(string, tag = "2")]
    pub client_id: String,
    #[prost(string, tag = "3")]
    pub price: String,
    #[prost(string, tag = "4")]
    pub quantity: String,
    #[prost(string, tag = "5")]
    pub amount: String,
    #[prost(string, tag = "6")]
    pub avg_price: String,
    #[prost(int32, tag = "7")]
    pub order_type: i32,
    #[prost(int32, tag = "8")]
    pub trade_type: i32,
    #[prost(bool, tag = "9")]
    pub is_maker: bool,
    #[prost(string, tag = "10")]
    pub remain_amount: String,
    #[prost(string, tag = "11")]
    pub remain_quantity: String,
    #[prost(string, optional, tag = "12")]
    pub last_deal_quantity: Option<String>,
    #[prost(string, tag = "13")]
    pub cumulative_quantity: String,
    #[prost(string, tag = "14")]
    pub cumulative_amount: String,
    #[prost(int32, tag = "15")]
    pub status: i32,
    #[prost(int64, tag = "16")]
    pub create_time: i64,
    #[prost(string, optional, tag = "17")]
    pub market: Option<String>,
    #[prost(int32, optional, tag = "18")]
    pub trigger_type: Option<i32>,
    #[prost(string, optional, tag = "19")]
    pub trigger_price: Option<String>,
    #[prost(int32, optional, tag = "20")]
    pub state: Option<i32>,
    #[prost(string, optional, tag = "21")]
    pub oco_id: Option<String>,
    #[prost(string, optional, tag = "22")]
    pub route_factor: Option<String>,
    #[prost(string, optional, tag = "23")]
    pub symbol_id: Option<String>,
    #[prost(string, optional, tag = "24")]
    pub market_id: Option<String>,
    #[prost(string, optional, tag = "25")]
    pub market_currency_id: Option<String>,
    #[prost(string, optional, tag = "26")]
    pub currency_id: Option<String>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicBookTickerV3Api {
    #[prost(string, tag = "1")]
    pub bid_price: String,
    #[prost(string, tag = "2")]
    pub bid_quantity: String,
    #[prost(string, tag = "3")]
    pub ask_price: String,
    #[prost(string, tag = "4")]
    pub ask_quantity: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PrivateDealsV3Api {
    #[prost(string, tag = "1")]
    pub price: String,
    #[prost(string, tag = "2")]
    pub quantity: String,
    #[prost(string, tag = "3")]
    pub amount: String,
    #[prost(int32, tag = "4")]
    pub trade_type: i32,
    #[prost(bool, tag = "5")]
    pub is_maker: bool,
    #[prost(bool, tag = "6")]
    pub is_self_trade: bool,
    #[prost(string, tag = "7")]
    pub trade_id: String,
    #[prost(string, tag = "8")]
    pub client_order_id: String,
    #[prost(string, tag = "9")]
    pub order_id: String,
    #[prost(string, tag = "10")]
    pub fee_amount: String,
    #[prost(string, tag = "11")]
    pub fee_currency: String,
    #[prost(int64, tag = "12")]
    pub time: i64,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PrivateAccountV3Api {
    #[prost(string, tag = "1")]
    pub vcoin_name: String,
    #[prost(string, tag = "2")]
    pub coin_id: String,
    #[prost(string, tag = "3")]
    pub balance_amount: String,
    #[prost(string, tag = "4")]
    pub balance_amount_change: String,
    #[prost(string, tag = "5")]
    pub frozen_amount: String,
    #[prost(string, tag = "6")]
    pub frozen_amount_change: String,
    #[prost(string, tag = "7")]
    pub r#type: String,
    #[prost(int64, tag = "8")]
    pub time: i64,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicSpotKlineV3Api {
    #[prost(string, tag = "1")]
    pub interval: String,
    #[prost(int64, tag = "2")]
    pub window_start: i64,
    #[prost(string, tag = "3")]
    pub opening_price: String,
    #[prost(string, tag = "4")]
    pub closing_price: String,
    #[prost(string, tag = "5")]
    pub highest_price: String,
    #[prost(string, tag = "6")]
    pub lowest_price: String,
    #[prost(string, tag = "7")]
    pub volume: String,
    #[prost(string, tag = "8")]
    pub amount: String,
    #[prost(int64, tag = "9")]
    pub window_end: i64,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicMiniTickerV3Api {
    #[prost(string, tag = "1")]
    pub symbol: String,
    #[prost(string, tag = "2")]
    pub price: String,
    #[prost(string, tag = "3")]
    pub rate: String,
    #[prost(string, tag = "4")]
    pub zoned_rate: String,
    #[prost(string, tag = "5")]
    pub high: String,
    #[prost(string, tag = "6")]
    pub low: String,
    #[prost(string, tag = "7")]
    pub volume: String,
    #[prost(string, tag = "8")]
    pub quantity: String,
    #[prost(string, tag = "9")]
    pub last_close_rate: String,
    #[prost(string, tag = "10")]
    pub last_close_zoned_rate: String,
    #[prost(string, tag = "11")]
    pub last_close_high: String,
    #[prost(string, tag = "12")]
    pub last_close_low: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicMiniTickersV3Api {
    #[prost(message, repeated, tag = "1")]
    pub items: Vec<PublicMiniTickerV3Api>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicBookTickerBatchV3Api {
    #[prost(message, repeated, tag = "1")]
    pub items: Vec<PublicBookTickerV3Api>,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicIncreaseDepthsBatchV3Api {
    #[prost(message, repeated, tag = "1")]
    pub items: Vec<PublicIncreaseDepthsV3Api>,
    #[prost(string, tag = "2")]
    pub event_type: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicAggreDepthsV3Api {
    #[prost(message, repeated, tag = "1")]
    pub asks: Vec<PublicAggreDepthV3ApiItem>,
    #[prost(message, repeated, tag = "2")]
    pub bids: Vec<PublicAggreDepthV3ApiItem>,
    #[prost(string, tag = "3")]
    pub event_type: String,
    #[prost(string, tag = "4")]
    pub from_version: String,
    #[prost(string, tag = "5")]
    pub to_version: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicAggreDepthV3ApiItem {
    #[prost(string, tag = "1")]
    pub price: String,
    #[prost(string, tag = "2")]
    pub quantity: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicAggreDealsV3Api {
    #[prost(message, repeated, tag = "1")]
    pub deals: Vec<PublicAggreDealsV3ApiItem>,
    #[prost(string, tag = "2")]
    pub event_type: String,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicAggreDealsV3ApiItem {
    #[prost(string, tag = "1")]
    pub price: String,
    #[prost(string, tag = "2")]
    pub quantity: String,
    #[prost(int32, tag = "3")]
    pub trade_type: i32,
    #[prost(int64, tag = "4")]
    pub time: i64,
}

#[derive(Clone, PartialEq, ::prost::Message)]
pub(crate) struct PublicAggreBookTickerV3Api {
    #[prost(string, tag = "1")]
    pub bid_price: String,
    #[prost(string, tag = "2")]
    pub bid_quantity: String,
    #[prost(string, tag = "3")]
    pub ask_price: String,
    #[prost(string, tag = "4")]
    pub ask_quantity: String,
}
