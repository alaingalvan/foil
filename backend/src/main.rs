#![warn(unused_extern_crates)]
#![warn(unused_crate_dependencies)]

mod github;
mod graphql;

use github::{github_hook, redirect};
use graphql::{graphql_handler, graphql_playground_handler, graphql_schema};

use axum::{
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Extension,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum::{extract::State, http::uri::Uri};
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};
use std::net::SocketAddr;
use tower_http::services::ServeDir;
type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;
use sqlx::ConnectOptions;
use sqlx::{postgres::PgConnectOptions, Pool, Postgres};
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;

use std::borrow::Cow;
use std::env;
use std::str::FromStr;
use std::time::Duration;

//=====================================================================================================================
/// Reverse proxy get requests to Node.js renderer.
async fn handler_renderer(
    State(client): State<Client>,
    mut req: Request<Body>,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    let path_query = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(path);

    let uri = format!("http://127.0.0.1:4011{}", path_query);
    *req.uri_mut() = Uri::try_from(uri).unwrap();

    Ok(client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}
//=====================================================================================================================
/// Error handling for the foil server.
async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("Request timed out."));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("Foil service is overloaded, try again later..."),
        );
    }
    // This is a serious server error:
    println!("{}", error);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from("Unhandled internal error"),
    )
}

//=====================================================================================================================
/// Add default headers to most responses.
async fn add_headers(req: Request<Body>, next: Next) -> Result<Response, Response> {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert("x-download-options", "noopen".parse().unwrap());
    headers.insert("x-frame-options", "SAMEORIGIN".parse().unwrap());
    headers.insert("x-dns-prefetch-control", "off".parse().unwrap());
    headers.insert("x-permitted-cross-domain-policies", "none".parse().unwrap());
    headers.insert("x-xss-protection", "0".parse().unwrap());
    headers.insert(
        "referrer-policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    headers.remove("x-powered-by");
    headers.remove("server");
    Ok(response)
}

//=====================================================================================================================
// Backend server main.
#[tokio::main]
async fn main() {
    // 🧻 Start logger
    env_logger::init();
    // 📚 Configure Database
    let pg_url = env::var("FOIL_DATABASE_URL")
        .expect("Fatal Error: No environment var FOIL_DATABASE_URL found.");
    let opts = PgConnectOptions::from_str(&pg_url)
        .unwrap()
        .log_statements(log::LevelFilter::Trace);
    let postgres_pool: Pool<Postgres> = Pool::connect_with(opts)
        .await
        .expect("Fatal Error: Cannot connect to database.");

    let args: Vec<String> = env::args().collect();
    let mut serve_assets_dir = "assets";
    if args.len() > 1 {
        serve_assets_dir = &args[args.len() - 1];
    }
    println!("Using asset directory: {serve_assets_dir}");

    let serve_assets = ServeDir::new(serve_assets_dir);

    // 🎒 Create Backend Server

    let client: Client =
        hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new());
    let app = Router::new()
        // API Endpoints
        .route("/api/v1/github/", get(redirect).post(github_hook))
        // 📊 GraphQL
        .route(
            "/api/v1/graphql",
            get(graphql_playground_handler).post(graphql_handler),
        )
        // ⚡ Frontend Assets / Backend Static Files
        .nest_service("/assets", serve_assets)
        // ⚛️ Single Page Application HTML Template
        .fallback(get(handler_renderer))
        .layer(Extension(graphql_schema(&postgres_pool)))
        .layer(Extension(postgres_pool.clone()))
        .layer(ServiceBuilder::new().layer(middleware::from_fn(add_headers)))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_error))
                .timeout(Duration::from_secs(300))
                .layer(TraceLayer::new_for_http()),
        )
        .with_state(client);

    // ✨ Bind Foil Backend:
    println!("✨ Foil Backend Server running in http://localhost:4017");
    let addr = SocketAddr::from(([127, 0, 0, 1], 4017));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.layer(TraceLayer::new_for_http()))
        .await
        .unwrap();
}
