# Contributing

`mkt` is a Rust workspace for exchange adapters and shared trading types.

## Before sending changes

1. Run `cargo fmt`.
2. Run `cargo clippy --workspace --all-features --all-targets -- -D warnings`.
3. Run `cargo test --workspace --all-features`.

## Code rules

- Keep public APIs deliberate; prefer `#[non_exhaustive]` for extensible public types.
- Preserve exchange error payloads when mapping external failures.
- Do not add default features that pull in exchange adapters.
- Keep new time fields on `time::OffsetDateTime`.

## Scope

Small adapter fixes, type model changes, docs improvements, and test coverage are all welcome. For larger API changes, open an issue first so the surface can stay stable.
