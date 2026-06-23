# svelte-rust-test

A full-stack web application where a single Rust/Axum binary serves both prerendered SvelteKit pages and JSON API endpoints. It includes SSO login via OIDC (Dex for local dev), Postgres-backed sessions, role-based access control, and API key authentication for service accounts.

## Architecture overview

```
Browser
  │
  └─► Axum (port 3000)
        ├── /auth/**       → OIDC login flow (Rust handlers)
        ├── /api/**        → JSON API (Rust handlers, file-based routing)
        └── /**            → build/  (SvelteKit prerendered static output)
```

One server process. Rust handles everything. The SvelteKit side is compiled to plain HTML/CSS/JS before the server starts — there is no Node.js process at runtime.

---

## Local development setup

### Prerequisites

- [Podman](https://podman.io/) with the WSL machine running
- [podman-compose](https://github.com/containers/podman-compose)
- Rust toolchain (`cargo`)
- Node.js + npm (the `openapi-typescript` devDependency generates the TS API
  types from the OpenAPI spec; installed by `npm install`, no global tooling)

### 1. Start the database and identity provider

```powershell
.\scripts\db-start.ps1
```

This starts two containers:

| Container | Port | Purpose |
|---|---|---|
| Postgres 17 | 5432 | Session store, user records, API keys |
| Dex v2.41.1 | 5556 | Local OIDC identity provider |

Data persists in named Docker volumes (`pgdata`, `dexdata`) between restarts. To stop:

```powershell
.\scripts\db-stop.ps1
```

### 2. Configure environment variables

```powershell
Copy-Item .env.example .env
```

The defaults in `.env.example` work with the local Podman setup as-is. See [Environment variables](#environment-variables) for details.

### 3. Run the database migrations

Migrations are a manual step — they do **not** run on startup. Apply them once (and again after pulling new migrations):

```powershell
cargo run -- migrate
```

This applies any pending migrations and exits.

### 4. Run the application

```powershell
cargo run
```

`cargo build` compiles the Svelte frontend first (via `build.rs`), then compiles the Rust binary. The app listens on `http://localhost:3000`.

---

## Authentication

### Web users — OIDC/SSO

All pages require login. Unauthenticated requests are redirected to `/auth/login`, which starts an OIDC flow with Dex (or any OIDC-compatible provider in production).

**Flow:** `/auth/login` → Dex login page → `/auth/callback` → session cookie set → redirected to `/`

Sessions are stored in Postgres with a 7-day expiry. The session cookie is encrypted with `axum-extra`'s `PrivateCookieJar`.

**Local test accounts** (configured in `dex/config.yaml`):

| Username | Password | Default role |
|---|---|---|
| `admin` | `admin` | `user` (promote to `admin` — see below) |
| `user1` | `user1` | `user` |
| `user2` | `user2` | `user` |

### Service accounts — API keys

API endpoints accept `Authorization: Bearer <token>` as an alternative to a session cookie. Tokens are prefixed `svrt_` and their SHA-256 hash is stored in the `api_keys` table — the plaintext is never persisted.

API keys carry their own role and are created through the [admin panel](#admin-panel).

---

## Role-based access control

Roles are stored in the `users` table and baked into the session at login time. Role changes take effect on the user's next login.

| Role | Access |
|---|---|
| `user` | All pages and `/api/*` endpoints |
| `admin` | Everything above + `/admin/` panel and admin API routes |

### Bootstrapping the first admin

Every user starts with `role = 'user'`. After the `admin` account logs in for the first time, promote it via psql:

```sql
UPDATE users SET role = 'admin' WHERE email = 'admin@example.com';
```

Then log out and back in. After that, admin management can be done through the UI.

---

## Admin panel

Available at `/admin/` to users with `role = 'admin'`. Provides:

- **View** all API keys (name, role, created, expires, last used)
- **Create** new API keys — choose a name, role, and optional expiry. The token is shown once at creation time.
- **Revoke** any key immediately

---

## OpenAPI / API docs

The API is documented automatically from the route tree. No annotations on handlers are needed — paths, HTTP methods, path parameters, and doc comments come straight from the file structure. Schema shapes come from `#[derive(utoipa::ToSchema)]` on response types and `#[derive(utoipa::IntoParams)]` on query-parameter structs.

| URL | Description |
|---|---|
| `GET /api/docs` | Swagger UI (requires login) |
| `GET /api/docs/openapi.json` | Raw OpenAPI 3.1 spec (requires login) |

The spec is generated once on first request (cached for the lifetime of the process) via `ApiRouter::openapi()`, emitted by the `openapi` flag on the `#[folder_router]` macro.

### Adding a new endpoint to the spec

1. Write the handler with a concrete return type — `-> Json<MyType>` rather than `-> impl IntoResponse`.
2. Derive `ToSchema` on the response type and `IntoParams` on any query-parameter struct.
3. Import those types at the `#[folder_router]` site in `main.rs` (the macro names them there when building the spec).
4. Doc comments on the handler function become the operation summary and description.

---

## Route tree

**SvelteKit** processes files whose names start with `+`. It ignores `route.rs`.

**axum-folder-router** processes files named exactly `route.rs`, plus `middleware.rs`, `fallback.rs`, and `intercept.rs`. It ignores everything else. The directory path maps directly to the URL — `[param]` folders become `:param` path parameters.

---

## Code layout

`route.rs` handlers stay thin: extract the request, check authorization, call a
service function, map the result to a response. All persistence and domain logic
lives in service modules **outside** `src/routes/`, so it can be reused (the
middleware and the admin handlers share the same API-key code) and read without
the routing noise.

---

## Security middleware

Auth is enforced by per-subtree `intercept.rs` files, not a single global middleware. Each intercept resolves the caller once and attaches a `Principal` extension; handlers read it without touching the database again.

---

## Health checks

Two unauthenticated probes, for use by orchestrators and load balancers:

| Route | Probe | Behaviour |
|---|---|---|
| `GET /health` | Liveness | Always `200 {"status":"ok"}` — does no I/O, so it stays green while a dependency is down. Restart the process only if this fails. |
| `GET /ready` | Readiness | `200 {"status":"ready"}` when Postgres answers a trivial query; `503 {"status":"unavailable"}` otherwise, so traffic is withheld until the DB is reachable. |

---

## Database migrations

Migrations are embedded at compile time with `sqlx::migrate!()`. They are applied manually — not on startup — so a deploy never silently alters the schema:

```powershell
cargo run -- migrate    # apply pending migrations, then exit
```

No live database is required to compile.

| Migration | Description |
|---|---|
| `0001_create_sessions.sql` | Sessions table |
| `0002_create_api_keys.sql` | API keys table |
| `0003_rbac.sql` | Users table, `role` column on sessions and api_keys |
| `0004_role_check.sql` | `CHECK` constraint restricting `role` to `user`/`admin` |

---

## Environment variables

Copy `.env.example` to `.env`. In production, set these in your environment directly — `dotenvy` is only enabled in debug builds.

| Variable | Description |
|---|---|
| `DATABASE_URL` | Postgres connection string |
| `OIDC_ISSUER` | OIDC provider base URL (Dex: `http://localhost:5556/dex`) |
| `OIDC_CLIENT_ID` | Client ID registered with the provider |
| `OIDC_CLIENT_SECRET` | Client secret |
| `OIDC_REDIRECT_URI` | Must match what's registered with the provider |
| `SESSION_SECRET` | Cookie encryption key — must be ≥ 64 characters |
| `HOST` | Bind address (default: `127.0.0.1`) |
| `PORT` | Port (default: `3000`) |

Postgres-specific variables (`POSTGRES_USER`, `POSTGRES_PASSWORD`, etc.) are only used by `compose.yml`.

---

## Build pipeline

`cargo build` compiles **only the Rust binary** — it no longer invokes Node, so the Rust build graph carries no frontend dependency. The SvelteKit output dir (`build/`) is hardcoded in [fallback.rs](src/routes/fallback.rs) and served lazily at runtime by `ServeDir`, so the bundle needn't exist to compile.

The cross-language pieces are built out-of-band by [`scripts/build.ps1`](scripts/build.ps1), in dependency order:

1. **build rust** — `cargo build` (produces the binary).
2. **build openapi** — `cargo run -- dump-openapi` writes `openapi.json` from the compiled route tree (no DB/config).
3. **build typescript** — `npm run gen:types` runs `openapi-typescript` over `openapi.json` → `src/lib/api/openapi.d.ts`.
4. **build svelte** — `npm run build` (Vite) writes the static bundle to `build/`. The SPA fallback (`fallback: 'index.html'` in `svelte.config.js`) generates `build/index.html`; unknown routes are handled client-side by `+error.svelte`.

`openapi.json` and `openapi.d.ts` are committed (so the frontend type-checks with no Rust toolchain). The `openapi_golden` test (`cargo test`) is a drift guard: it fails if `openapi.json` no longer matches the route tree. The same spec is also served live at `/api/docs/openapi.json`.

---

## Development workflow

```powershell
# Full rebuild (Rust + Svelte) then run
cargo run

# Frontend hot-reload while iterating on Svelte components:
# Terminal 1 — Rust API server
cargo run

# Terminal 2 — Vite dev server with HMR on :5173, proxies /api/* to Rust
npm run dev
```

Navigate to `http://localhost:5173` for HMR. Navigate to `http://localhost:3000` to test the fully compiled output.

---

## Observability

```powershell
# Adjust log level
$env:RUST_LOG = "info,tower_http=debug"
cargo run

# tokio-console task inspector (port 6669)
cargo run --features tokio-console
tokio-console   # second terminal
```

---

## Deployment

Copy the compiled binary next to the `build/` directory:

```
my-app/
├── svelte-rust-test.exe
└── build/
    ├── index.html
    ├── hello/index.html
    ├── admin/index.html
    └── _app/
```

Set all environment variables (no `.env` file in production) and run the binary. No Node.js, no reverse proxy, no separate static file server required.

---

## Key dependencies

| Crate | Role |
|---|---|
| `axum` | HTTP server and router |
| `axum-folder-router` | Compile-time file-based API routing (local fork); `openapi` feature generates spec |
| `axum-extra` | `PrivateCookieJar` for encrypted session cookies |
| `tower-http` | Static file serving, tracing, path normalization, compression |
| `tower_governor` | Rate limiting on `/auth` |
| `tokio` | Async runtime |
| `sqlx` | Async Postgres driver, embedded migrations |
| `openidconnect` | OIDC client (PKCE, nonce, token verification) |
| `utoipa` | OpenAPI schema generation (`ToSchema`, `IntoParams`); source for the TS API types |
| `sha2` / `hex` | API key hashing |
| `rand` | Cryptographically random API key generation |
| `serde` / `serde_json` | Serialization |
| `dotenvy` / `envy` | `.env` loading (debug only) and env → struct deserialization |
| `tracing` / `tracing-subscriber` | Structured logging |
| `console-subscriber` *(optional)* | tokio-console integration |
| `@sveltejs/kit` | File-based page routing, SSG |
| `@sveltejs/adapter-static` | SPA fallback + prerender pages to static HTML |
