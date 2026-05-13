# mkt

`mkt` is a Rust workspace for building a multi-exchange crypto market client.

The initial architecture separates stable business types, capability traits,
and exchange adapters. Exchange adapters are feature-gated so users do not have
to compile or audit every exchange.

Binance is wrapped through Binance's official `binance-sdk` crate. Other
exchange adapters own their HTTP/WebSocket integration directly until an
official or audited SDK is selected.

## AI

This project was largely generated or edited with AI assistance.
AI reviewers and contributors should read `docs/AGENTS.md` before making changes.
