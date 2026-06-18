use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// Crate-wide result alias. Handlers return `Result<T>` and use `?`; the
/// `IntoResponse` impl below turns any `Error` into the right HTTP response.
pub type Result<T> = std::result::Result<T, Error>;

/// Every failure a handler can return. Each variant maps to one HTTP status.
///
/// The rule for what reaches the client: 4xx messages are safe to echo (they
/// describe the caller's request); 5xx never leak internals — the detail is
/// logged server-side and the client gets a generic message.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Any database failure. → 500
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Missing or invalid credentials. → 401
    #[error("unauthorized")]
    Unauthorized,

    /// Authenticated, but not allowed to do this. → 403
    #[error("forbidden")]
    Forbidden,

    /// Malformed request the caller can fix; the message is shown to them. → 400
    #[error("{0}")]
    BadRequest(String),

    /// A failure in the OIDC handshake (token exchange, missing/invalid token). → 500
    #[error("authentication flow error: {0}")]
    Auth(String),
}

impl Error {
    fn status(&self) -> StatusCode {
        match self {
            Error::Database(_) | Error::Auth(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Error::Unauthorized => StatusCode::UNAUTHORIZED,
            Error::Forbidden => StatusCode::FORBIDDEN,
            Error::BadRequest(_) => StatusCode::BAD_REQUEST,
        }
    }

    /// The body sent to the client. Server errors are deliberately generic so
    /// internal detail (SQL, token internals) never escapes.
    fn client_message(&self) -> &str {
        match self {
            Error::BadRequest(msg) => msg,
            Error::Unauthorized => "unauthorized",
            Error::Forbidden => "forbidden",
            Error::Database(_) | Error::Auth(_) => "internal server error",
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status();
        // Log the full detail for anything that's our fault; the client sees only
        // the generic message from `client_message`.
        if status.is_server_error() {
            tracing::error!("{self}");
        }
        (status, self.client_message().to_string()).into_response()
    }
}
