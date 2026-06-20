# CLAUDE.md

Guidance for working in this repo. For human-onboarding detail (setup, auth flow,
deployment, env vars) see [README.md](README.md) — this file covers the
conventions and gotchas that aren't obvious from the code.

## What this is

A single Rust/Axum binary that serves both a prerendered SvelteKit app and a JSON
API. There is **no Node process at runtime** — the frontend is compiled to static
files in `build/` and served as the fallback. OIDC login (Dex locally),
Postgres-backed sessions, RBAC, and Bearer API keys.

Platform is **Windows / PowerShell**; containers run under **Podman**, not Docker.

## The routing model — read this first

`src/routes/` is shared by two file-based routers that ignore each other's files:

| File | Owner | Maps to |
|---|---|---|
| `+page.svelte`, `+layout.svelte`, `+layout.js` | SvelteKit | a page / layout |
| `route.rs` | `axum-folder-router` | an Axum handler at the folder's URL |
| `middleware.rs` | `axum-folder-router` | middleware over the folder's subtree |
| `fallback.rs` | `axum-folder-router` | fallback for unmatched paths in the subtree |

Folder path = URL. `[id]` folders become `:id` path params. The macro entry point
is `ApiRouter::into_router_with_state(state)` in [main.rs](src/main.rs).

### `axum-folder-router` is a local fork

`Cargo.toml` pins it to `{ path = "../axum-folder-router" }`. The
`middleware.rs`/`fallback.rs` conventions and the nest-based composition are
**custom to this fork** — they do not exist in the crates.io release. If routing
behavior needs to change at the convention level, edit the fork. **The macro's
own tests live in the fork** (`../axum-folder-router/tests`), not here.

### middleware.rs / fallback.rs have two forms

The macro picks the form by arity:

- **Stateless** — `fn middleware<S>(router: Router<S>) -> Router<S> where S: Clone + Send + Sync + 'static`
- **Stateful** — `fn middleware(router: Router<AppState>, state: AppState) -> Router<AppState>`

Use the stateful form only when the layer needs `AppState` at build time (e.g.
`from_fn_with_state`). Any stateful middleware/fallback anywhere in the tree makes
`into_router_with_state` the required entry point. `fallback.rs` follows the same
two-form rule with a `fallback(...)` function.

A subtree that has a `middleware.rs` **or** `fallback.rs` is a "boundary" and gets
`nest`ed at its prefix; plain subtrees stay flat-inlined. A catch-all (`[...rest]`)
folder **cannot** own a `middleware.rs`/`fallback.rs` (axum forbids wildcards in a
nest prefix) — the macro rejects it at compile time.

### Where the actual middleware lives in this app

The `*/middleware.rs` and `fallback.rs` files are **thin wrappers**; the logic is
in `crate::auth`. Current wiring (the README's "Security middleware" section
describes the older monolithic `require_login` and is out of date — trust the
code):

| File | Effect |
|---|---|
| [src/routes/middleware.rs](src/routes/middleware.rs) | global: request-id, tracing, body limit, compression, security headers, timeout |
| [src/routes/auth/middleware.rs](src/routes/auth/middleware.rs) | rate limiting on `/auth` (`tower_governor`) |
| [src/routes/api/middleware.rs](src/routes/api/middleware.rs) | `require_api_auth` — Bearer or session, else 401 |
| [src/routes/api/admin/middleware.rs](src/routes/api/admin/middleware.rs) | `require_admin` — role check |
| [src/routes/fallback.rs](src/routes/fallback.rs) | serves `build/` behind `require_page_auth` (session → redirect) |

`/auth`, `/health`, `/ready` live outside `/api`, so they need no auth carve-out.

## Code conventions

- **Handlers stay thin.** `route.rs` extracts the request, checks authz, calls a
  service function, maps the result. All SQL/domain logic lives in service modules
  **outside** `src/routes/` (`src/auth/*`). Don't put `sqlx` queries in handlers —
  in this codebase every query is in `src/auth/db.rs`, reached through services.
- **Roles** resolve once in middleware into a `Principal` (request extension).
  Handlers read `Extension<Principal>`; they never re-read the session or hit the
  `users` table.
- **Errors** go through `crate::error::Error` / `Result` with one `IntoResponse`
  impl ([error.rs](src/error.rs)). Return `Error`, don't hand-roll responses.

## Commands

```powershell
cargo install typeshare-cli # one-time: required on PATH for type generation
cargo check                 # fast iteration — does NOT build the frontend
cargo run                   # build.rs generates TS types + builds the Svelte frontend, then runs (:3000)
cargo run -- migrate        # apply pending migrations, then exit (NEVER auto-runs)
npm run dev                 # Vite HMR on :5173, proxies /api/* to Rust (run alongside cargo run)
npm run gen:types           # regenerate src/lib/api/generated.ts from the #[typeshare] DTOs
.\scripts\db-start.ps1      # start Postgres + Dex (Podman)
.\scripts\db-stop.ps1
```

- Prefer `cargo check` while iterating — `cargo build`/`run` triggers
  [build.rs](build.rs), which runs `npm run build` whenever a frontend file
  changed (tracked via `.frontend-stamp`).
- **Migrations are manual.** Startup never touches the schema. After adding a
  migration, the user runs `cargo run -- migrate`. No live DB is needed to compile
  (`sqlx::migrate!` embeds them).
- To test macro/routing changes, `cargo test` in `../axum-folder-router`.
- Type-check the frontend: `npm run check` (runs `svelte-check`).

## Shared types

API DTOs are plain `serde` structs in the handlers/services that own them (e.g.
[src/auth/api_keys.rs](src/auth/api_keys.rs),
[src/routes/api/me/route.rs](src/routes/api/me/route.rs)). Each is annotated with
`#[typeshare]`, and the [`typeshare`](https://github.com/1Password/typeshare) CLI
generates the matching TypeScript into **`src/lib/api/generated.ts`** (a
generated file — committed so the frontend type-checks without a Rust
toolchain, but regenerated on every build; do not edit by hand).
[src/lib/api/client.ts](src/lib/api/client.ts) re-exports those types and holds
the hand-written `fetch` wrappers + `ApiError` that the Svelte files import from
`$lib/api/client`.

- **Generation runs automatically** as the first step of `build.rs` (so a plain
  `cargo build` regenerates types → builds the frontend → compiles the binary),
  and via `npm run gen:types` for Node-only flows. `npm run dev`/`check` run it
  first. **Prerequisite:** `cargo install typeshare-cli` (like `npm`, it must be
  on `PATH`). The driver lives in the `svelte-rust` crate
  (`build::typescript_types`, [../svelte-rust/src/lib.rs](../svelte-rust/src/lib.rs)).
- **Only `#[typeshare]`-bearing `.rs` files get `rerun-if-changed`** — editing
  other Rust files won't regenerate or rebuild the frontend, preserving fast
  `cargo check`. Adding a `#[typeshare]` DTO to a *new* file is picked up
  automatically (the driver scans `src/`).
- **`time::OffsetDateTime` and `Uuid` → `string`** via `[typescript.type_mappings]`
  in [typeshare.toml](typeshare.toml) (RFC3339 / hyphenated-UUID serialisation);
  parse dates with `new Date(iso)`.
- **`Option<T>` becomes an optional `field?: T`** (`string | undefined`), *not*
  `string | null` — even though serde serialises `None` as JSON `null`. Use
  falsy checks (`if (!x)`) at call sites; don't compare `=== null`.

## Gotchas

- **Timeout layer must stay innermost** in the `ServiceBuilder` chain in
  [src/routes/middleware.rs](src/routes/middleware.rs): `tower_http`'s
  `TimeoutLayer` emits a `Default` (empty) body, which only the router's bare
  `Body` satisfies — not the composed compression/limit body. Reordering it
  outward breaks the build.
- **No CSP** is set. A strict CSP must be coordinated with SvelteKit's own `csp`
  config, so it's deliberately left to the app — don't add a blanket CSP header
  in the middleware without that coordination.
- **Rate-limit IP extraction.** `/auth` uses `tower_governor` with
  `SmartIpKeyExtractor` (XFF-leftmost → X-Real-Ip → peer). Behind a k8s Ingress
  the leftmost XFF entry is client-controlled (spoofable) and the peer address is
  the Ingress pod — so behind a proxy this needs a trusted-hop extractor
  (rightmost XFF). `ConnectInfo<SocketAddr>` is wired in `main` for the peer
  fallback (local dev).
- **`route_layer` vs `layer`.** The auth/admin guards use `route_layer` so they
  don't run on unmatched paths (which fall through to the fallback). Keep that
  distinction when adding subtree middleware.
- `Secure` cookies and HSTS only engage in release builds / over HTTPS — local
  `http://localhost` works in debug. See `secure_cookie` in
  [src/auth/mod.rs](src/auth/mod.rs).
