## 2026-06-30

### Axum — No Batteries-Included Web Framework

Axum is a minimal Rust web framework. It gives you routing, extractors, and middleware — nothing else. No ORM, no templating, no auth. You compose what you need on top.

Compared to other Rust frameworks:

- **Actix Web** — heavier, its own actor runtime, own middleware system, more built-in
- **Loco.rs** — full Rails-style, batteries-included (SeaORM, auth, workers, mailer, CLI scaffolding)
- **Rocket** — macro-heavy, built-in form handling, templating

### Tower Layers (Middleware)

Tower is Axum's middleware system. A **layer** wraps the app and intercepts every request/response — like Express middleware in Node or Django middleware in Python.

```rust
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tower_http::compression::CompressionLayer;

let app = Router::new()
    .route("/", get(hello_world))
    .layer(CompressionLayer::new())   // gzip responses
    .layer(CorsLayer::permissive())   // allow cross-origin
    .layer(TraceLayer::new());        // log every request/response
```

Layers run **bottom-to-top on request**, **top-to-bottom on response**. It's composable — you only ship what you use, no magic.

Common Tower layers:

| Crate | Layer | Does |
|-------|-------|------|
| `tower-http` | `TraceLayer` | Request/response logging |
| `tower-http` | `CorsLayer` | CORS headers |
| `tower-http` | `CompressionLayer` | gzip/brotli responses |
| `tower-http` | `TimeoutLayer` | Request timeout |
| `tower-http` | `LimitLayer` | Rate limiting |
| `tower` | `ConcurrencyLimitLayer` | Max concurrent requests |
| `tower-http` | `AuthLayer` | Auth guard (via `ValidateRequest`) |
| `tower` | `BufferLayer` | Backpressure handling |
| `tower-http` | `SensitiveHeadersLayer` | Masks cookies/auth headers in logs |

### Rust Persistence / ORM Landscape

Three main players for database access in Rust:

| Crate | Stars | Style | Async | Best With |
|-------|-------|-------|-------|-----------|
| **SQLx** | ~17k | Write SQL, compile-time checked | ✅ | Axum — same minimal philosophy |
| Diesel | ~14k | Schema DSL, query builder | ❌ (needs `spawn_blocking`) | Sync apps, most mature |
| SeaORM | ~10k | Entities, relations, ActiveRecord | ✅ | When you want a "real ORM" |

Go's `jmoiron/sqlx` is a different project — same name, completely different scope (thin `database/sql` wrapper vs. full async driver + migrations + compile-time query checking).

### SQLx + SQLite with Axum

**Setup:**
```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "migrate"] }
```

**Migrations** live in `migrations/` and run via `sqlx::migrate!().run(&pool).await`. File naming determines the type:

| Pattern | Type | Behavior |
|---------|------|----------|
| `*.sql` | Simple | One-way, forward only |
| `*.up.sql` | Reversible up | Forward migration (has matching `*.down.sql`) |
| `*.down.sql` | Reversible down | Revert migration (has matching `*.up.sql`) |

Example migration up:
```sql
CREATE TABLE IF NOT EXISTS counters (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    value INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO counters (id, name, value) VALUES (1, 'visits', 0);
```

Example migration down:
```sql
DROP TABLE IF EXISTS counters;
```

**`sqlx::migrate!()` only runs forward** (up) migrations. To revert, install `sqlx-cli` and run `sqlx migrate revert` — it picks the right file by the `*.down.sql` suffix. No magic comment separators inside a single file.

**Sharing state** — Axum uses `.with_state(pool)` to inject the connection pool into handlers:
```rust
let app = Router::new()
    .route("/", get(handler))
    .with_state(pool);
```

**Extracting state** in handlers:
```rust
async fn handler(State(pool): State<SqlitePool>) -> String { ... }
```

**⚠️ SQLite connection URL gotcha:** SQLite needs the file to exist before SQLx can open it, even for a new database. The fix: call `std::fs::File::create(&db_path)` before connecting. Also, the URL format matters: `sqlite:///absolute/path` (three slashes) for absolute paths, `sqlite:relative/path` for relative.

**Atomic counter pattern** — UPDATE then SELECT, no transaction needed for a simple hit counter:
```rust
sqlx::query("UPDATE counters SET value = value + 1 WHERE name = 'visits'")
    .execute(&pool).await?;
let (count,) = sqlx::query_as("SELECT value FROM counters WHERE name = 'visits'")
    .fetch_one(&pool).await?;
```

### SQLx Compile-Time Query Checking

Two API tiers — unchecked (runtime) vs checked (compile-time):

| Function | Verifies at build? | When it fails |
|----------|-------------------|---------------|
| `sqlx::query()` / `sqlx::query_as()` | ❌ No | Runtime — `Result::Err` |
| `sqlx::query!()` / `sqlx::query_as!()` | ✅ Yes | Won't compile |

**How it works:**
1. Set `DATABASE_URL` env var (or `.env` file) pointing to a real database
2. At build time, the `!` macros connect to that database
3. They run `EXPLAIN` / introspect the schema
4. Wrong column name? → **compile error**, not a runtime panic
5. Wrong type? → **compile error**

Example — this won't compile:
```rust
// Compile error: no such column: nonexistent
let row = sqlx::query!("SELECT nonexistent FROM counters WHERE name = 'visits'")
    .fetch_one(&pool).await?;
```

`.env` file:
```
DATABASE_URL=sqlite:///home/loller/dev/rust-lol/db.sqlite
```

**Tradeoffs:**

**The `.env` file:**

SQLx macros have built-in dotenv support — they automatically look for a `.env` file in the project root. No `dotenv` crate needed, and **it's only for the macros at build time**, not for the running app:

```
# .env
DATABASE_URL=sqlite:///home/loller/dev/rust-lol/db.sqlite
```

Our app constructs the URL manually in `main.rs` at runtime (`std::env::current_dir().join("db.sqlite")`), so the `.env` is purely a build-time helper for the `!` macros. To make the app itself read `.env` at runtime, you'd add the `dotenv` crate and call `dotenv::dotenv().ok();` at startup.
- Pro: catches SQL bugs at build time, no runtime surprises
- Con: needs a running DB with up-to-date schema at build time
- Workaround: `SQLX_OFFLINE=true` uses a pre-generated schema cache file (`.sqlx/`) — no DB needed in CI

**`query!()` returns auto-derived structs** — field names come from column names:
```rust
let row = sqlx::query!("SELECT value FROM counters WHERE name = 'visits'")
    .fetch_one(&pool).await?;
// row.value is i64 (type inferred from the actual column)
```

**`query_as!()` needs a struct type** — more explicit, same checking:
```rust
#[derive(sqlx::FromRow)]
struct Counter { value: i64 }

let row = sqlx::query_as!(Counter, "SELECT value FROM counters WHERE name = 'visits'")
    .fetch_one(&pool).await?;
```

**What macros can't check:**
- Dynamic SQL (string-built queries, `IN (...)` lists) — fall back to `sqlx::query()`
- Custom extensions SQLx doesn't understand — usually passes through if syntax is parsable
- Queries where the schema changes at runtime — macros only see the DB at build time

### Testing with SQLx + Axum

**In-memory SQLite** is the killer feature for tests — no file, no cleanup, instant:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&pool).await.unwrap(); // migrations work on :memory: too
        pool
    }

    #[tokio::test]
    async fn counter_increments() {
        let pool = test_pool().await;

        let body1 = handler(State(pool.clone())).await;
        assert_eq!(body1, "Hello, world! — you are visitor #1");

        let body2 = handler(State(pool.clone())).await;
        assert_eq!(body2, "Hello, world! — you are visitor #2");

        let body3 = handler(State(pool)).await;
        assert_eq!(body3, "Hello, world! — you are visitor #3");
    }
}
```

Key pieces:
- `sqlite::memory:` — each connection gets a fresh, isolated database; nothing persists to disk
- `sqlx::migrate!()` still works on `:memory:` — schema is set up exactly like production
- `#[tokio::test]` — needed because handlers are async
- `pool.clone()` is cheap — `SqlitePool` is just an `Arc` internally, so cloning doesn't create new connections
- **Test handlers directly** — call the function with `State(pool)` instead of going through HTTP. Faster, simpler, no port binding. For full HTTP integration tests, use `axum_test` or `reqwest` + spawn the router
- `#[cfg(test)]` — the module only compiles during `cargo test`, zero cost at build time
