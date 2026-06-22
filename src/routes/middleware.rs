//! Global middleware, applied at the routes-folder root so it wraps every route
//! and the static fallback. Stateless — none of these layers need `AppState`.

use std::time::Duration;

use axum::{
    Router,
    http::{HeaderValue, StatusCode, header},
};
use tower::ServiceBuilder;
use tower_http::{
    compression::CompressionLayer,
    limit::RequestBodyLimitLayer,
    normalize_path::NormalizePathLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

// The API only takes small JSON payloads; cap bodies so a client can't stream an
// arbitrarily large request.
const BODY_LIMIT: usize = 2 * 1024 * 1024;
// Abort a request that runs longer than this (`tower_http`'s layer responds 408
// rather than erroring, so it stays axum-compatible).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub fn middleware<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    // Layer order is outer-to-inner. `TimeoutLayer` must be innermost: it emits
    // an empty (`Default`) body on timeout, which only the router's plain `Body`
    // satisfies — not the composed compression/limit body types. The security
    // headers sit outside it so even a 408/413 response carries them.
    router.layer(
        ServiceBuilder::new()
            // Strip trailing slashes before routing so `/api/docs/` matches
            // `/api/docs`. Runs first (outermost) so no other layer ever sees
            // the un-normalized path.
            .layer(NormalizePathLayer::trim_trailing_slash())
            // Tag each request with an `x-request-id` (generated if absent) and
            // echo it back so logs and clients can correlate. Set before the
            // trace layer so spans capture the id.
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(TraceLayer::new_for_http())
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(RequestBodyLimitLayer::new(BODY_LIMIT))
            .layer(CompressionLayer::new())
            // Security headers. No CSP here — a strict CSP must be coordinated
            // with SvelteKit's own `csp` config, so it's left to the app.
            .layer(SetResponseHeaderLayer::overriding(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_FRAME_OPTIONS,
                HeaderValue::from_static("DENY"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::REFERRER_POLICY,
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            ))
            // Browsers ignore HSTS over plain HTTP, so it's safe to always set;
            // it only takes effect once served over HTTPS.
            .layer(SetResponseHeaderLayer::overriding(
                header::STRICT_TRANSPORT_SECURITY,
                HeaderValue::from_static("max-age=31536000; includeSubDomains"),
            ))
            .layer(TimeoutLayer::with_status_code(
                StatusCode::REQUEST_TIMEOUT,
                REQUEST_TIMEOUT,
            )),
    )
}
