# mkt-exchange-mexc

`mkt-exchange-mexc` is the MEXC adapter crate for `mkt`.

## Mainnet Smoke Test

The crate includes an ignored live smoke test for MEXC spot `USDCUSDT`. It
covers public REST market data, public WebSocket decoding, account balances,
market buy, order lookup, fills, market sell, and open-order cleanup checks.

The test never reads a `.env` file itself. Export credentials into the test
process environment and explicitly opt in to live trading:

```sh
MEXC_MAINNET_API_KEY=... \
MEXC_MAINNET_SECRET_KEY=... \
MKT_MEXC_MAINNET_SMOKE=1 \
cargo test -p mkt-exchange-mexc --test mainnet_smoke -- --ignored --nocapture
```

The order flow spends at most `1.10 USDT`, applies a balance safety factor, and
skips live orders when the available balance is below the venue-reported minimum
notional. The API key must have MEXC spot account and trading permissions.
