# mkt

`mkt` is the facade crate for the workspace.

Use this crate when you want a stable entrypoint that re-exports the shared
types and core capability traits, plus optional exchange adapters behind
feature flags.

## Features

- `binance`
- `bitget`
- `bybit`
- `mexc`
- `okx`
- `all-exchanges`

## Dependencies

`mkt` depends on `mkt-core` and `mkt-types`, and optionally on individual
exchange adapter crates.
