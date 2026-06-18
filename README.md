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
- Node.js + npm

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

### 3. Run the application

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

## Route tree

```
src/routes/
├── +layout.js                    ← SvelteKit: prerender config
├── +layout.svelte                ← SvelteKit: global auth guard (client-side)
├── +page.svelte                  ← SvelteKit: home page
├── hello/
│   └── +page.svelte              ← SvelteKit: /hello page
├── admin/
│   └── +page.svelte              ← SvelteKit: admin panel UI
├── auth/
│   ├── login/route.rs            ← Axum: GET  /auth/login  (start OIDC flow)
│   ├── callback/route.rs         ← Axum: GET  /auth/callback
│   └── logout/route.rs           ← Axum: POST /auth/logout
└── api/
    ├── hello/route.rs            ← Axum: GET /api/hello
    ├── me/route.rs               ← Axum: GET /api/me
    └── admin/
        └── api_keys/
            ├── route.rs          ← Axum: GET + POST /api/admin/api_keys
            └── [id]/
                └── route.rs      ← Axum: DELETE /api/admin/api_keys/:id
```

**SvelteKit** processes files whose names start with `+`. It ignores `route.rs`.

**axum-folder-router** processes files named exactly `route.rs`. It ignores everything else. The directory path maps directly to the URL — `[param]` folders become `:param` path parameters.

---

## Code layout

`route.rs` handlers stay thin: extract the request, check authorization, call a
service function, map the result to a response. All persistence and domain logic
lives in service modules **outside** `src/routes/`, so it can be reused (the
middleware and the admin handlers share the same API-key code) and read without
the routing noise.

```
src/
├── main.rs                 ← config, app wiring, startup (spawns the session reaper)
├── error.rs                ← `enum Error` + `Result` alias + one `IntoResponse` impl
└── auth/
    ├── mod.rs              ← Principal, secure_cookie(), require_login middleware
    ├── sessions.rs         ← create / find / delete sessions + background reaper
    ├── api_keys.rs         ← bearer authentication + admin list/create/delete
    ├── users.rs            ← user upsert on login
    └── oidc.rs             ← begin() login URL + complete() code exchange & verify
```

A handler like `GET /api/admin/api_keys` is then just an admin guard plus
`api_keys::list(&state.db).await?`; the SQL, hashing, and token generation are in
[`auth/api_keys.rs`](src/auth/api_keys.rs).

---

## Security middleware

Every request passes through `require_login` before reaching a handler:

```
Request
  │
  ├── /auth/* or /_app/*  →  pass through (public)
  │
  ├── Bearer token present?
  │     ├── valid API key  →  attach Principal{role}, continue
  │     └── invalid        →  401
  │
  └── session cookie present?
        ├── valid session  →  attach Principal{role}, continue
        ├── no session + /api/*  →  401
        └── no session + page   →  redirect /auth/login
```

Handlers that need a role check extract `Extension<Principal>` and call `principal.is_admin()`. No handler re-reads the session or hits the `users` table — the role is resolved once in middleware.

---

## Database migrations

Migrations are embedded at compile time with `sqlx::migrate!()` and run automatically on startup. No live database is required to compile.

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

`cargo build` compiles both halves:

1. `build.rs` detects changes to `.svelte`, `.js`, `.ts`, `.css`, or `.html` files under `src/routes/`, plus `svelte.config.js`, `vite.config.js`, and `package.json`.
2. If `node_modules/` is missing it runs `npm install` first.
3. Runs `npm run build`, writing prerendered output to `build/`.
4. Writes `.frontend-stamp` so Cargo skips the frontend build when nothing changed.

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
| `axum-folder-router` | Compile-time file-based API routing |
| `axum-extra` | `PrivateCookieJar` for encrypted session cookies |
| `tower-http` | Static file serving, request tracing |
| `tokio` | Async runtime |
| `sqlx` | Async Postgres driver, embedded migrations |
| `openidconnect` | OIDC client (PKCE, nonce, token verification) |
| `sha2` / `hex` | API key hashing |
| `rand` | Cryptographically random API key generation |
| `serde` / `serde_json` | Serialization |
| `dotenvy` / `envy` | `.env` loading (debug only) and env → struct deserialization |
| `tracing` / `tracing-subscriber` | Structured logging |
| `console-subscriber` *(optional)* | tokio-console integration |
| `@sveltejs/kit` | File-based page routing, SSG |
| `@sveltejs/adapter-static` | Prerender all pages to static HTML |
