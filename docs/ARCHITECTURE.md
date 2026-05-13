# mkt Architecture

## Overview

`mkt` is a multi-exchange Rust client for REST and WebSocket workflows. It presents a stable strategy-facing API while keeping exchange-specific protocol details inside adapter crates.

The system is organized around capability traits, stable business types, and exchange adapters. Strategies interact with capabilities such as market data, spot trading, futures trading, account access, and streams. Adapters translate those capabilities into each exchange's SDK or protocol.

```text
strategy / application
        |
        v
  mkt facade crate
        |
        v
  mkt-core capability traits
        |
        v
  exchange adapters
        |
        v
official SDKs / exchange REST / exchange WebSocket
```

## Workspace

```text
crates/
  mkt/
  mkt-types/
  mkt-core/
  exchanges/
    binance/
    bybit/
    bitget/
    mexc/
    okx/
```

`mkt` is the facade crate. It provides the user-facing entrypoint, common re-exports, feature-gated exchange modules, and the prelude.

`mkt-types` contains stable business data structures shared by strategies and adapters.

`mkt-core` contains capability traits, exchange handles, configuration, capability metadata, stream abstractions, and shared errors.

`crates/exchanges/*` contains one adapter crate per exchange.

## Layering

```text
┌────────────────────────────────────────────┐
│ Strategy / Application Code                │
│ Uses stable types and capability traits     │
└──────────────────────┬─────────────────────┘
                       │
┌──────────────────────▼─────────────────────┐
│ mkt Facade                                 │
│ Re-exports core/types/adapters              │
└──────────────────────┬─────────────────────┘
                       │
┌──────────────────────▼─────────────────────┐
│ mkt-core                                   │
│ Capability traits, ExchangeHandle, errors   │
└──────────────────────┬─────────────────────┘
                       │
┌──────────────────────▼─────────────────────┐
│ Exchange Adapters                          │
│ SDK/protocol integration and DTO mapping    │
└──────────────────────┬─────────────────────┘
                       │
┌──────────────────────▼─────────────────────┐
│ Exchange SDKs / REST / WebSocket APIs       │
└────────────────────────────────────────────┘
```

The architecture unifies exchange access at the capability layer. HTTP clients, WebSocket clients, signing, generated DTOs, and SDK lifecycle details remain inside adapters.

## Type Model

`mkt-types` defines the shared domain model used across REST responses, WebSocket events, requests, and strategy code.

Main type groups:

- Exchange identity: `ExchangeId`, `KnownExchange`
- Markets and symbols: `Market`, `MarketKind`, `Symbol`, `SpotSymbol`, `FuturesSymbol`
- Public market data: `LastPrice`, `OrderBook`, `Kline`, `Trade`
- Trading: `Order`, `Fill`, `SpotOrderRequest`, `FuturesOrderRequest`
- Account state: `Balance`, `Position`
- Extension data: `Extensions`, `ExtensionField<T>`, `ExtensionSchema`

The type model separates spot and futures request types, models numeric business values with decimal precision, and keeps exchange-specific fields in typed extensions.

## Capability Model

`mkt-core` exposes exchange functionality through small capability traits.

`ExchangeInfo` provides exchange identity and capability metadata.

`MarketData` represents REST public market data: markets, tickers, order books, trades, and klines.

`SpotTrading` represents spot order placement, cancellation, lookup, open orders, and fills.

`FuturesTrading` represents derivatives order workflows, fills, positions, and leverage updates.

`Account` represents account-level state such as balances.

`PublicStream` and `PrivateStream` represent WebSocket subscription entrypoints.

Capability traits do not inherit `ExchangeInfo`. Exchange identity belongs to the handle-level info component; market data, trading, account, and stream components only implement their own behavior.

## ExchangeHandle

`ExchangeHandle` is a facade over capability trait objects.

```text
ExchangeHandle
  info: dyn ExchangeInfo
  market_data: Option<dyn MarketData>
  spot_trading: Option<dyn SpotTrading>
  futures_trading: Option<dyn FuturesTrading>
  account: Option<dyn Account>
  public_stream: Option<dyn PublicStream>
  private_stream: Option<dyn PrivateStream>
```

Strategies receive `ExchangeHandle` values or direct trait objects. The handle provides uniform capability lookup without exposing concrete adapter types.

## Adapter Architecture

Each exchange adapter owns the integration with one exchange.

```text
adapter client
  ├── exchange configuration
  ├── official SDK or direct protocol clients
  ├── authentication and signing integration
  ├── REST DTO mapping
  ├── WebSocket DTO mapping
  ├── capability trait implementations
  └── raw exchange access where available
```

Adapters expose user-facing client types that own or reference the exchange integration. Internally, adapters may split work into capability components that share adapter state. Those components implement only the capabilities they support and are assembled into `ExchangeHandle` by adapter-owned constructors such as `into_handle()`.

Adapters map exchange-native DTOs into `mkt-types`. Adapter internals may differ by exchange. An adapter backed by an official SDK delegates protocol coverage and connection internals to that SDK; a direct adapter owns its concrete HTTP/WebSocket clients.

`Capabilities` describes the public surface of an adapter:

```text
Capabilities
  exchange_id
  markets
  rest capabilities
  stream capabilities
  transport control
```

`TransportControl` describes I/O ownership:

```text
OfficialSdkManaged { sdk }
DirectManaged
```

## REST Flow

```text
strategy
  -> MarketData / SpotTrading / FuturesTrading / Account
  -> exchange adapter
  -> SDK call or exchange REST request
  -> exchange DTO
  -> mkt-types domain value
  -> strategy
```

REST capability methods return stable domain values. Adapter-specific response fields are attached through typed extensions when they are useful to callers but not part of the shared model.

## WebSocket Flow

```text
strategy
  -> PublicStream / PrivateStream
  -> exchange adapter subscription
  -> SDK stream or exchange WebSocket connection
  -> exchange event DTO
  -> MarketDataEvent / PrivateEvent
  -> PublicEventStream / PrivateEventStream
  -> strategy
```

`PublicSubscription` and `PrivateSubscription` describe requested stream topics. `PublicEventStream` and `PrivateEventStream` provide the unified async event interface.

## Extension Architecture

`Extensions` carries exchange-specific fields in the domain model without expanding every shared type with exchange-only fields.

```text
Extensions
  BTreeMap<ExtensionKey, ExtensionValue>

ExtensionKey
  namespace.name

ExtensionValue
  Text
  Decimal
  Boolean
  Bytes
```

Typed extension fields are exported by adapter crates:

```text
mkt_exchange_binance::ext::PREVENTED_MATCH_ID
mkt_exchange_binance::ext::PREVENTED_QUANTITY
```

`ExtensionSchema` groups typed fields into adapter-owned schemas for request and response extension surfaces.

## Raw Access

Adapters can expose raw exchange access through `raw()` when they have a meaningful exchange-native surface.

```text
client.raw()
  -> official SDK clients
  -> native signed REST operations
  -> native WebSocket subscriptions
```

Raw access sits beside the unified capability model. It gives callers a path to exchange-native functionality that is outside the stable `mkt-types` model.

## Facade API

The `mkt` crate collects the public entrypoints:

```text
mkt::core
mkt::types
mkt::exchanges::{binance, bybit, bitget, mexc, okx}
mkt::prelude
```

Exchange modules are feature-gated. Strategy code can depend only on `mkt::prelude` and receive exchange handles from application wiring.

## Extensibility

Third-party exchanges integrate by implementing `mkt-core` capability traits over their own client type.

```text
third-party adapter
  -> implements ExchangeInfo
  -> implements supported capabilities
  -> maps native DTOs to mkt-types
  -> exports typed extensions for exchange-specific fields
  -> can be wrapped in ExchangeHandle
```

The extension point is the capability trait boundary. New adapters and new capability implementations do not require changes to existing strategies.

## Error Boundary

`mkt-core` defines the shared error type used across capability traits.

```text
MktError
  network
  timeout
  authentication
  exchange rejected
  invalid request
  capability missing
  serialization
  transport/protocol
```

Adapters convert SDK and protocol errors into this shared error boundary while preserving exchange-specific context where relevant.
