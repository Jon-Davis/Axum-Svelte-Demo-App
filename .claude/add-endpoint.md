# Adding a Typed, Documented API Endpoint

This guide walks through adding a new endpoint. Utoipa derives drive **both** the
OpenAPI docs and the generated TypeScript types — there's a single source of
truth (no Typeshare). The OpenAPI schema is emitted to `openapi.json` and
`openapi-typescript` turns it into `src/lib/api/openapi.d.ts`.

## Anatomy of an Endpoint

```rust
// src/routes/api/example/route.rs
use axum::Json;
use serde::{Deserialize, Serialize};

// Request: query params, body, path params, etc.
#[derive(Deserialize, utoipa::IntoParams)]
pub struct ExampleQuery {
    /// Description for OpenAPI docs
    pub param_name: Option<String>,
}

// Response: ToSchema drives both the OpenAPI schema and the generated TS type
#[derive(Serialize, utoipa::ToSchema)]
pub struct ExampleResponse {
    /// Description for OpenAPI docs
    pub result: String,
}

/// Endpoint summary for OpenAPI (this doc comment shows up in /api/docs)
pub async fn get(
    Query(params): Query<ExampleQuery>,
) -> Json<ExampleResponse> {
    Json(ExampleResponse {
        result: "data".to_string(),
    })
}
```

## Step-by-Step

### 1. Create the route file

Create `src/routes/api/<path>/route.rs`. Folder path becomes the URL:
- `src/routes/api/users/route.rs` → `GET /api/users`
- `src/routes/api/items/[id]/route.rs` → `GET /api/items/:id`

### 2. Define request type (if needed)

```rust
#[derive(Deserialize, utoipa::IntoParams)]
pub struct MyQuery {
    /// Shown in OpenAPI docs
    pub field: Option<String>,
}
```

Use `utoipa::IntoParams` for query/path parameters. Add `///` doc comments—they appear in `/api/docs`.

### 3. Define response type

```rust
#[derive(Serialize, utoipa::ToSchema)]
pub struct MyResponse {
    pub data: String,
}
```

`#[derive(Serialize, utoipa::ToSchema)]` is all you need — it feeds both the
OpenAPI schema and (via `openapi.json` → `openapi-typescript`) the TS type.

For optional fields, just use `Option<T>`:
```rust
#[derive(Serialize, utoipa::ToSchema)]
pub struct MyResponse {
    pub required: String,
    pub optional: Option<String>,  // → `optional?: string | null` in TS
}
```

### 4. Write the handler

```rust
/// Brief description of what this endpoint does.
/// This shows up as the operation summary in OpenAPI docs.
pub async fn get(Query(params): Query<MyQuery>) -> Json<MyResponse> {
    // Extract, validate, call service, map result
    Json(MyResponse { data: "...".to_string() })
}

// For POST/PUT: use axum extractors (Json, Path, Extension<Principal>, etc.)
// For DELETE: return StatusCode
pub async fn delete(Path(id): Path<Uuid>) -> StatusCode {
    StatusCode::NO_CONTENT
}
```

### 5. Handle authentication

Use request extensions and guards:
```rust
use axum::Extension;
use crate::auth::Principal;

pub async fn get(
    Extension(principal): Extension<Principal>,
    // ... other extractors
) -> Json<MyResponse> {
    if !principal.is_admin() {
        // Return error via crate::error::Error
    }
    // ...
}
```

Or add an `intercept.rs` at the folder for the whole subtree. See [src/routes/api/admin/intercept.rs](../../src/routes/api/admin/intercept.rs) for an example.

## Type Generation & OpenAPI

The `axum-folder-router` macro with the `openapi` feature generates an `OpenApi`
object from the entire route tree; `Utoipa` derives (`IntoParams`, `ToSchema`)
populate the schema for each operation. Everything downstream flows from that one
document. After writing the handler:

1. **Regenerate the spec:** `cargo run -- dump-openapi` writes `openapi.json`
   from the compiled route tree (no DB/config). The `openapi_golden` test in
   [main.rs](../../src/main.rs) is a drift guard — `cargo test` fails if
   `openapi.json` is stale, so a forgotten regen can't pass CI.

2. **Regenerate the TS types:** `npm run gen:types` runs `openapi-typescript`
   over `openapi.json` → `src/lib/api/openapi.d.ts`. Both files are committed so
   the frontend type-checks with no Rust toolchain.

   (Or just run [`scripts/build.ps1`](../../scripts/build.ps1), which does both
   plus the rust/svelte builds in order.)

3. **OpenAPI docs** are served from the route tree at `/api/docs` (schemas, doc-comment
   summaries, parameter descriptions, error responses).

## Example: POST with Body

```rust
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateItemRequest {
    /// Item name (required)
    pub name: String,
    /// Item description (optional)
    pub description: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ItemResponse {
    pub id: Uuid,
    pub name: String,
}

/// Create a new item
pub async fn post(
    State(state): State<&'static AppState>,
    Json(body): Json<CreateItemRequest>,
) -> Json<ItemResponse> {
    let item = crate::services::items::create(&state.db, &body).await?;
    Json(ItemResponse {
        id: item.id,
        name: item.name,
    })
}
```

## Gotchas

- **Request params use `IntoParams`**, responses use `ToSchema`. Don't mix them.
- **Optional fields (`Option<T>`) become `field?: T | null`** in TypeScript (optional *and* nullable). Use falsy checks (`if (!x)`), which cover both `undefined` and `null`.
- **Date/UUID serialization:** `OffsetDateTime`/`Uuid` carry an OpenAPI `format` and render as `string` in TS (RFC3339 / hyphenated). Parse with `new Date(iso)`.
- **Regenerate after changing a DTO:** `cargo run -- dump-openapi` then `npm run gen:types` (or `scripts/build.ps1`) — otherwise the committed types lag the Rust structs. `cargo test` will fail until you do.
- **All queries go in `src/auth/db.rs`**, not in handlers. Handlers call service functions.

## Testing

```bash
cargo check                  # Fast—Rust only
cargo run -- dump-openapi     # Regenerate openapi.json from the route tree
npm run gen:types            # openapi.json → openapi.d.ts
.\scripts\build.ps1          # Or: full build (rust → openapi → ts → svelte)
curl http://localhost:3000/api/docs  # View OpenAPI UI
```
