# rust-lol

A minimal Axum web server with a SQLite-backed visitor hit counter.

## Quick Start

```bash
# Build
cargo build

# Run (starts at http://127.0.0.1:3000)
cargo run
```

## Migrations

Migrations run automatically on startup via `sqlx::migrate!()`. For manual control, install the SQLx CLI:

```bash
cargo install sqlx-cli --no-default-features -F rustls,sqlite

# Run forward migrations (db file must exist first)
touch db.sqlite
sqlx migrate run --database-url sqlite://$(pwd)/db.sqlite

# Revert the last migration
sqlx migrate revert --database-url sqlite://$(pwd)/db.sqlite
```
