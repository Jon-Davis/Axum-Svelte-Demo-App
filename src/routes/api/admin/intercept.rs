//! Authorization for the `/api/admin` subtree: admins only. The outer `/api`
//! intercept has already authenticated the caller and inserted their `Principal`
//! into the request extensions, so this is a pure role check — a textbook
//! intercept that only ever diverts.
//!
//! Stateless (one parameter): the `Principal` is read from the request the outer
//! intercept mutated, so no app state is needed here. Unlike the old
//! `middleware.rs` (which used `route_layer`), an intercept is always `.layer`ed,
//! so this guard also covers unmatched `/api/admin/*` paths instead of letting
//! them fall through to the static fallback.

use std::ops::ControlFlow;

use axum::{
    extract::Request,
    response::{IntoResponse, Response},
};

use crate::auth::Principal;
use crate::error::Error;

pub async fn intercept(req: Request) -> ControlFlow<Response, Request> {
    match req.extensions().get::<Principal>() {
        Some(p) if p.is_admin() => ControlFlow::Continue(req),
        _ => ControlFlow::Break(Error::Forbidden.into_response()),
    }
}
