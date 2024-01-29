#![warn(unused_extern_crates)]
#![warn(unused_crate_dependencies)]

mod github;
mod graphql;

use github::{github_hook, redirect};
use glob::Pattern;
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
use std::{net::SocketAddr, path::PathBuf};
type Client = hyper_util::client::legacy::Client<HttpConnector, Body>;
use sqlx::ConnectOptions;
use sqlx::{postgres::PgConnectOptions, Pool, Postgres};
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;

use std::borrow::Cow;
use std::env;
use std::str::FromStr;
use std::time::Duration;

use lexiclean::Lexiclean;
use path_slash::PathBufExt;

fn clean_path_string(p: &std::path::PathBuf) -> String {
    p.clone()
        .lexiclean()
        .to_slash()
        .unwrap()
        .to_string()
        .replace("\\", "/")
}

/// State used for the server-side renderer and static asset router.
#[derive(Debug, Clone)]
struct RendererState {
    pub client: Client,
    pub pool: Pool<Postgres>,
}

//=====================================================================================================================
/// Reverse proxy get requests to Node.js renderer.
async fn handler_renderer(
    State(state): State<RendererState>,
    mut req: Request<Body>,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    let path_pathbuf = PathBuf::from(path);
    let ext_result = path_pathbuf.extension();
    let mut last_response = Response::<Body>::new("".into());
    *last_response.status_mut() = StatusCode::NOT_FOUND;

    // All paths that lack an extension are delegated to the node server-side renderer.
    if ext_result.is_none() {
        let path_query = req
            .uri()
            .path_and_query()
            .map(|v| v.as_str())
            .unwrap_or(path);

        let uri = format!("http://127.0.0.1:4011{}", path_query);
        *req.uri_mut() = Uri::try_from(uri).unwrap();

        return Ok(state
            .client
            .request(req)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?
            .into_response());
    }

    // We try to resolve paths that have an extension using what's exposed from foil modules relative to the path.
    let mut path_ancestors = path_pathbuf.ancestors();
    path_ancestors.next();
    loop {
        let ancestor = path_ancestors.next();
        match ancestor {
            Some(par) => {
                let par_path_buf = par.to_path_buf();
                let par_clean = clean_path_string(&par_path_buf);
                let cur_query = include_str!("graphql/sql/post_recursive.sql");
                // (Root Path, Permalink, Assets, Main)
                let sql_result: Result<(String, String, Vec<String>, String), sqlx::Error> =
                    sqlx::query_as(&cur_query)
                        .bind(&par_clean)
                        .fetch_one(&state.pool)
                        .await;
                match sql_result {
                    Ok(v) => {
                        // 🤍 Early out based on allowed paths and extensions.
                        // We first check if the foil main is the path, then check our whitelist.
                        let mut can_serve = v.3 == path;
                        if !can_serve {
                            for asset in v.2 {
                                let full_asset_path_buf = PathBuf::from(&v.1).join(&asset);
                                let full_asset_path = clean_path_string(&full_asset_path_buf);
                                match Pattern::new(&full_asset_path) {
                                    Ok(pat) => {
                                        can_serve |= pat.matches(&path);
                                        break;
                                    }
                                    Err(_) => (),
                                }
                            }
                        }
                        if !can_serve {
                            return Ok(last_response);
                        }

                        // 🫚 Split the permalink from the current request path:
                        // Example: /blog/ray-tracing-denoising/assets/cover.jpg becomes:
                        // Result: asset/cover.jpg
                        let mut cur_path_string = path.to_string().replacen(&v.1, "", 1);
                        if cur_path_string.starts_with("/") {
                            cur_path_string = cur_path_string.replacen("/", "", 1);
                        }
                        match PathBuf::from_str(&v.0) {
                            Ok(post_root) => {
                                let possible_file_path = post_root.join(&cur_path_string);
                                let mut svc =
                                    tower_http::services::ServeFile::new(possible_file_path);
                                let svc_resp = svc.try_call(Request::new(Body::empty())).await;
                                match svc_resp {
                                    Err(_svc_e) => (),
                                    Ok(re) => {
                                        return Ok(re.into_response());
                                    }
                                }
                            }
                            Err(_e) => (),
                        }
                    }
                    Err(_sql_e) => (),
                }
            }
            None => {
                break;
            }
        }
    }

    // We couldn't find a file due to a server error, so we 404 and redirect to the 404 frontend page:
    return Ok(last_response);
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

    // 🎒 Create Backend Server
    let renderer_state = RendererState {
        client: hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new()),
        pool: postgres_pool.clone(),
    };

    let app = Router::new()
        // API Endpoints
        .route("/api/v1/github/", get(redirect).post(github_hook))
        // 📊 GraphQL
        .route(
            "/api/v1/graphql",
            get(graphql_playground_handler).post(graphql_handler),
        )
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
        .with_state(renderer_state);

    // ✨ Bind Foil Backend:
    println!("✨ Foil Backend Server running in http://localhost:4017");
    let addr = SocketAddr::from(([127, 0, 0, 1], 4017));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app.layer(TraceLayer::new_for_http()))
        .await
        .unwrap();
}
