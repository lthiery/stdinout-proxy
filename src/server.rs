use super::*;
use axum::{extract::Query, http::StatusCode, response, routing::get, Extension, Router};
use axum_auth::AuthBearer;
use daemon_handle::DaemonHandle;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

pub type HandlerResult = std::result::Result<response::Json<Value>, (StatusCode, String)>;

#[derive(Debug, Clone, clap::Args)]
pub struct Server {}

impl Server {
    pub async fn run(self) -> Result {
        let daemon: Arc<Mutex<DaemonHandle>> = Arc::new(Mutex::new(DaemonHandle::new().await?));

        let app = if let Ok(env_token) = std::env::var("AUTH_TOKEN") {
            Router::new()
                .route("/v1/stdin", get(auth_stdin_handler))
                .layer(Extension(daemon))
                .layer(Extension(env_token))
        } else {
            Router::new()
                .route("/v1/stdin", get(stdin_handler))
                .layer(Extension(daemon))
        };

        let server_endpoint = std::env::var("PORT").unwrap_or("3000".to_string());
        println!("Binding to port {}...", server_endpoint);
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], server_endpoint.parse().unwrap()));
        tokio::select!(
            result = axum::Server::bind(&addr)
                .serve(app.into_make_service()) =>
                    result.map_err(|e| Error::Axum(e.into())),
        )
    }
}

impl From<Error> for HandlerResult {
    fn from(e: Error) -> Self {
        Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
    }
}

pub async fn auth_stdin_handler(
    Extension(daemon): Extension<Arc<Mutex<DaemonHandle>>>,
    Extension(env_token): Extension<String>,
    query: Query<daemon_handle::Params>,
    AuthBearer(token): AuthBearer

) -> HandlerResult {
    if token != env_token {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()));
    }
    stdin_handler(Extension(daemon), query).await
}

pub async fn stdin_handler(
    Extension(daemon): Extension<Arc<Mutex<DaemonHandle>>>,
    query: Query<daemon_handle::Params>,
) -> HandlerResult {
    let mut daemon = daemon.lock().await;
    let query = query.0;
    daemon
        .run(query)
        .await
        .map(|r| {
            response::Json(json!({
                "status": "success",
                "data": r}))
        })
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}