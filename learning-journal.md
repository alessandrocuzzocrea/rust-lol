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

### OpenAPI / Swagger with utoipa

**utoipa** is the stable choice for Axum 0.8 + OpenAPI. `aide` (the Axum-native alternative) is stuck on Axum 0.7 as of mid-2026.

**Dependencies:**
```toml
utoipa = { version = "5", features = ["axum_extras"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

**Step 1 — Mark your types** with `ToSchema`:
```rust
#[derive(Serialize, ToSchema)]
struct User {
    id: u32,
    name: String,
    email: String,
}
```

**Step 2 — Define the OpenAPI document** as a struct:
```rust
#[derive(OpenApi)]
#[openapi(
    paths(hello, get_user),
    components(schemas(User))
)]
struct ApiDoc;
```

**Step 3 — Annotate each handler** with `#[utoipa::path(...)]`:
```rust
#[utoipa::path(
    get,
    path = "/user/{id}",
    responses(
        (status = 200, description = "User found", body = User),
        (status = 404, description = "User not found")
    ),
    params(
        ("id" = u32, Path, description = "User ID")
    )
)]
async fn get_user(Path(id): Path<u32>) -> impl IntoResponse { ... }
```

**Step 4 — Merge Swagger UI into the router:**
```rust
let app = Router::new()
    .route("/", get(hello))
    .route("/user/{id}", get(get_user))
    .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()));
```

This gives you:
- `GET /docs` — interactive Swagger UI
- `GET /api-docs/openapi.json` — raw OpenAPI 3.1 JSON spec

**Types default to `IntoResponse`** — utoipa infers response schemas from the `body = User` annotation and the handler's return type. For complex cases, describe each status explicitly.

**Hardcoded data with `LazyLock`** — for prototypes, no DB needed:
```rust
static USERS: LazyLock<HashMap<u32, User>> = LazyLock::new(|| {
    HashMap::from([
        (1, User { id: 1, name: "Alice".into(), email: "alice@example.com".into() }),
    ])
});
```

`LazyLock` is lazy, thread-safe, and runs once on first access — perfect for static reference data.

### What is Tokio?

Rust doesn't ship with an async runtime — it ships with the *language primitives* (`async fn`, `.await`) but the thing that actually runs them is an external crate. **Tokio** is that crate.

**The stack, bottom to top:**

```
tokio              ← async runtime (event loop, task scheduler, I/O)
  └── hyper        ← HTTP protocol implementation (built on tokio)
       └── axum    ← web framework (routing, extractors, ergonomics on top of hyper)
            └── your code
```

**Tokio provides:**

| What | Example in our code |
|------|-------------------|
| **Event loop** | `#[tokio::main]` starts it, polls all tasks |
| **TCP listener** | `tokio::net::TcpListener::bind("0.0.0.0:3000")` |
| **Task spawner** | `tokio::spawn(async { ... })` — run background work |
| **Timers** | `tokio::time::sleep(Duration::from_secs(5))` |
| **I/O channels** | `tokio::sync::mpsc` — async message passing |
| **Blocking thread pool** | `tokio::task::spawn_blocking(...)` — for sync-heavy work |

**Why you need it:**

`async fn` by itself does nothing — it's just syntax. Something has to call `poll()` on every pending task, sleep while waiting, and wake tasks when their I/O is ready. That's Tokio's job:

```rust
#[tokio::main]  // ← This macro sets up Tokio's event loop
async fn main() {
    // Everything here runs inside Tokio
    // .await yields control back to the runtime
}
```

Without a `#[tokio::main]` (or manually calling `tokio::runtime::Runtime::block_on`), you can't run async code at all. The compiler gives you:

```
error: `await` is only allowed inside `async` functions and blocks
```

**Axum's relationship to Tokio:**

Axum doesn't do networking itself — it hands a router to `axum::serve(listener, app)`, which internally uses `hyper` for HTTP parsing and `tokio` for the TCP socket. Every request handler runs as a Tokio task. This is why SQLx (also Tokio-native) slots in seamlessly — they share the same runtime, same thread pool, same `.await` model.

**Recap:** Tokio is the engine, Axum is the steering wheel, hyper is the transmission. You can swap Axum for Actix or Warp, but Tokio remains the de facto standard async engine underneath most of the Rust web ecosystem.

### Sync vs Async in Rust

**The core idea:** sync code blocks the OS thread until it's done. Async code *yields* the thread while waiting, so one thread can juggle thousands of tasks.

**Concrete example — reading from a database:**

```
SYNC (Diesel style):
  Thread #12: [ask DB for row] -------WAITING------- [get row, continue]
  Thread #12 is frozen doing nothing for 5ms. The OS can't use it for anything else.

ASYNC (SQLx style):
  Task on Thread #12: [ask DB for row] .await
  Thread #12: [immediately switches to handle another request]
  ... later, DB responds ...
  Thread #8: [picks up the result, continues the task]
```

**Why this matters for a web server:**

A web server handles many concurrent connections. With sync:
- Each connection needs its own OS thread (expensive — ~8MB stack each)
- 10,000 connections = 10,000 threads = system melts

With async:
- One thread pool (typically N = CPU cores) juggles all connections
- 10,000 connections on 8 threads — normal operation
- Each `.await` point is a chance to switch to another task

**How Rust implements it:**

No runtime ships with the language. You pick one:

| Runtime | Used by | Trait |
|---------|---------|-------|
| **Tokio** | Axum, SQLx, hyper | `tokio::main` |
| async-std | Less common | `async_std::main` |
| smol | Lightweight use cases | `smol::block_on` |

Our stack is Tokio all the way down:
```rust
#[tokio::main]  // Tokio runtime, not std::main
async fn main() {
    // Axum runs on Tokio
    // SQLx pool runs on Tokio
    // .await yields to whichever task needs the thread
}
```

**The Diesel problem:**

Diesel is synchronous — it blocks the thread during every query:

```rust
// ❌ This blocks Tokio's thread #12 for 5ms
//    Other requests waiting on that thread are stuck
let users = users::table.load(&mut conn)?;

// ✅ Wrap in spawn_blocking — moves to a separate blocking thread pool
let users = tokio::task::spawn_blocking(move || {
    users::table.load(&mut conn)
}).await??;  // double ?? : spawn_blocking Result + Diesel Result
```

This is why Diesel + Axum is annoying. Every DB call gets wrapped. SQLx avoids this by being async-native — it uses Tokio's non-blocking I/O to talk to the database.

**The trade:**

Async isn't free:
- More complex stack traces (`.await` splits across tasks)
- `Send + Sync + 'static` bounds everywhere (the compiler enforces task-safety)
- Colored functions: async fns can only be called from other async fns (or a runtime)

But for web servers, the thread-per-connection model tops out fast. Async scales to orders of magnitude more concurrent connections on the same hardware.
