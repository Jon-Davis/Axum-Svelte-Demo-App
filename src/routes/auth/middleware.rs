//! Rate limiting for the `/auth` subtree. Auth endpoints have low legitimate
//! request rates, so a strict per-IP limit here blunts credential brute-forcing
//! without throttling the SPA's asset/API request bursts elsewhere. Stateless —
//! the limiter holds its own state.

use std::time::Duration;

use axum::Router;
use tower_governor::{
    GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor,
};

pub fn middleware<S>(router: Router<S>) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let config = GovernorConfigBuilder::default()
        .per_second(2)
        .burst_size(5)
        // Behind a reverse proxy: read X-Forwarded-For / X-Real-Ip, falling back
        // to the peer address (which needs `ConnectInfo`, set in `main`).
        .key_extractor(SmartIpKeyExtractor)
        .finish()
        .expect("valid governor config");

    // Periodically evict stale per-IP buckets so memory doesn't grow unbounded.
    let limiter = config.limiter().clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            limiter.retain_recent();
        }
    });

    router.layer(GovernorLayer::new(config))
}
