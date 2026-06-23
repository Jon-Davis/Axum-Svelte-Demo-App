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

## The routing model

`src/routes/` is shared by two file-based routers:

| File | Owner | Effect |
|---|---|---|
| `+page.svelte`, `+layout.svelte`, `+layout.js` | SvelteKit | page / layout |
| `route.rs` | `axum-folder-router` | Axum handler at folder's URL |
| `middleware.rs` | `axum-folder-router` | middleware over subtree |
| `intercept.rs` | `axum-folder-router` | per-request guard (inspect/allow/deny) |
| `fallback.rs` | `axum-folder-router` | fallback for unmatched paths |

Folder path = URL; `[id]` becomes `:id` param. Entry point: `ApiRouter::into_router_with_state(state)` in [main.rs](src/main.rs).

**Local fork:** `axum-folder-router` is at `../axum-folder-router` with custom `middleware.rs`/`fallback.rs`/`intercept.rs` conventions not in the crates.io release. Tests live in the fork.

**Stateless vs stateful forms:** The macro picks by arity. Stateless: `fn middleware<S>(router: Router<S>) -> Router<S>`. Stateful: `fn middleware(router: Router<&'static AppState>, state: &'static AppState) -> Router<&'static AppState>`. Use stateful only when `AppState` is needed at build time (e.g. `from_fn_with_state`). `fallback.rs` follows the same pattern.

**Boundaries:** A subtree with `middleware.rs`, `fallback.rs`, or `intercept.rs` gets nested at its prefix; plain subtrees stay flat. Catch-all folders (`[...rest]`) cannot own a boundary file.

**Intercept guards:** `intercept.rs` returns `ControlFlow<Response, Request>`; `Continue(req)` proceeds (can insert extensions like `Principal`), `Break(resp)` short-circuits. Attached with `.layer` (not `route_layer`), so it runs on the subtree's routes **and** fallback — unmatched `/api/*` returns 401, not fallback.

**Current guards:**

| File | Effect |
|---|---|
| [src/routes/middleware.rs](src/routes/middleware.rs) | request-id, tracing, body limit, compression, security headers, timeout |
| [src/routes/auth/middleware.rs](src/routes/auth/middleware.rs) | rate limiting on `/auth` |
| [src/routes/api/intercept.rs](src/routes/api/intercept.rs) | authenticate `/api` (Bearer or session), attach `Principal`, else 401 |
| [src/routes/api/admin/intercept.rs](src/routes/api/admin/intercept.rs) | admin-only on `/api/admin`, else 403 |
| [src/routes/admin/intercept.rs](src/routes/admin/intercept.rs) | redirect non-admins from `/admin` to `/forbidden` |
| [src/routes/fallback.rs](src/routes/fallback.rs) | serve `build/` with session auth (redirect if no session) |

`/auth`, `/health`, `/ready` are outside `/api` and need no auth carve-out.

## Code conventions

- **Thin handlers:** Extract request → check authz → call service → map result. All SQL/domain logic lives in `src/auth/*`, not handlers. Queries: `src/auth/db.rs`.
- **App state:** `&'static AppState` (leaked once in `main`). Handlers take `State<&'static AppState>`; per-request clones are pointer copies. The macro is invoked with `&'static AppState`.
- **Roles:** Resolved once in middleware into `Principal` (request extension). Handlers read `Extension<Principal>`; never re-fetch session/roles.
- **Errors:** Return `crate::error::Error` / `Result`; single `IntoResponse` impl in [error.rs](src/error.rs). No hand-rolled responses.

## Commands

```powershell
cargo check                 # fast iteration; Rust only
cargo run                   # run (:3000), serving the prebuilt build/ — does NOT build the frontend
cargo run -- migrate        # apply migrations (manual, never auto-runs)
cargo run -- dump-openapi   # regenerate openapi.json from the route tree (no DB/config)
.\scripts\build.ps1         # full deployable build: rust → openapi → ts types → svelte static
cargo test                  # openapi_golden drift guard (fails if openapi.json is stale)
npm run gen:types           # openapi.json → src/lib/api/openapi.d.ts (openapi-typescript)
npm run check               # type-check frontend (svelte-check)
.\scripts\db-start.ps1      # start Postgres + Dex (Podman)
.\scripts\db-stop.ps1       # stop
cargo test -p axum-folder-router  # test routing macro
```

- The frontend is built **out-of-band** by [scripts/build.ps1](scripts/build.ps1), not by `cargo build` — the Rust build carries no Node dependency. The bundle dir (`build/`) is hardcoded in [fallback.rs](src/routes/fallback.rs) and served lazily at runtime by `ServeDir`, so it needn't exist to compile. Run `scripts/build.ps1` after changing frontend code or an API DTO.
- **Type-gen flow:** API types flow Rust DTOs → `openapi.json` (`cargo run -- dump-openapi`) → `openapi.d.ts` (`npm run gen:types`). Both are committed; the `openapi_golden` test (`cargo test`) fails if `openapi.json` drifts from the route tree.
- Migrations are manual; startup never touches the schema. Migrations embed at compile time via `sqlx::migrate!`.

**Adding endpoints:** see [.claude/add-endpoint.md](.claude/add-endpoint.md) for the full guide on adding typed, documented routes with Typeshare (TypeScript generation) and Utoipa (OpenAPI docs).

