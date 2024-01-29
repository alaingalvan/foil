mod async_graphql_axum;
mod posts;

use async_graphql::dataloader::DataLoader;
use async_graphql::http::{playground_source, GraphQLPlaygroundConfig};
use async_graphql::{EmptyMutation, EmptySubscription, Schema};
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
use axum::{
    extract::Extension,
    http::header::LOCATION,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use axum_macros::debug_handler;
use posts::{FoilLoader, QueryPosts};
use sqlx::{Pool, Postgres};

pub fn queries() -> QueryPosts {
    posts::QueryPosts::default()
}

pub type FigSchema = Schema<QueryPosts, EmptyMutation, EmptySubscription>;

// 📈 Create the main Foil GraphQL Schema
pub fn graphql_schema(postgres_pool: &Pool<Postgres>) -> FigSchema {
    Schema::build(queries(), EmptyMutation, EmptySubscription)
        .data(DataLoader::new(
            FoilLoader::new(postgres_pool.clone()),
            tokio::spawn,
        ))
        .data(postgres_pool.clone())
        .finish()
}

// 📊 Main GraphQL Handler endpoint.
#[debug_handler]
pub async fn graphql_handler(schema: Extension<FigSchema>, req: GraphQLRequest) -> GraphQLResponse {
    let gql_inner = req.into_inner();
    let gql_response = schema.execute(gql_inner).await;
    gql_response.into()
}

// 🧸 Main GraphQL Handler endpoint.
pub async fn graphql_playground_handler() -> Response {
    if cfg!(debug_assertions) {
        Html(playground_source(GraphQLPlaygroundConfig::new("/"))).into_response()
    } else {
        (StatusCode::TEMPORARY_REDIRECT, [(LOCATION, "/404")]).into_response()
    }
}
