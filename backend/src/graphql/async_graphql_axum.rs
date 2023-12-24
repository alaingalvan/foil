use std::{io::ErrorKind, marker::PhantomData, str::FromStr};

use async_graphql::{futures_util::TryStreamExt, http::MultipartOptions, ParseRequestError};
use axum::{
    body::Body,
    extract::{FromRequest, Request},
    http::{self, HeaderValue, Method},
    response::{IntoResponse, Response},
};
use tokio_util::compat::TokioAsyncReadCompatExt;

/// Extractor for GraphQL request.
pub struct GraphQLRequest<R = rejection::GraphQLRejection>(
    pub async_graphql::Request,
    PhantomData<R>,
);

impl<R> GraphQLRequest<R> {
    /// Unwraps the value to `async_graphql::Request`.
    #[must_use]
    pub fn into_inner(self) -> async_graphql::Request {
        self.0
    }
}

/// Rejection response types.
pub mod rejection {
    use async_graphql::ParseRequestError;
    use axum::{
        body::Body,
        http,
        http::StatusCode,
        response::{IntoResponse, Response},
    };

    /// Rejection used for [`GraphQLRequest`](GraphQLRequest).
    pub struct GraphQLRejection(pub ParseRequestError);

    impl IntoResponse for GraphQLRejection {
        fn into_response(self) -> Response {
            match self.0 {
                ParseRequestError::PayloadTooLarge => http::Response::builder()
                    .status(StatusCode::PAYLOAD_TOO_LARGE)
                    .body(Body::empty())
                    .unwrap(),
                bad_request => http::Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!("{:?}", bad_request)))
                    .unwrap(),
            }
        }
    }

    impl From<ParseRequestError> for GraphQLRejection {
        fn from(err: ParseRequestError) -> Self {
            GraphQLRejection(err)
        }
    }
}

#[async_trait::async_trait]
impl<S, R> FromRequest<S> for GraphQLRequest<R>
where
    S: Send + Sync,
    R: IntoResponse + From<ParseRequestError>,
{
    type Rejection = R;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        Ok(GraphQLRequest(
            GraphQLBatchRequest::<R>::from_request(req, state)
                .await?
                .0
                .into_single()?,
            PhantomData,
        ))
    }
}

/// Extractor for GraphQL batch request.
pub struct GraphQLBatchRequest<R = rejection::GraphQLRejection>(
    pub async_graphql::BatchRequest,
    PhantomData<R>,
);

impl<R> GraphQLBatchRequest<R> {
    /// Unwraps the value to `async_graphql::BatchRequest`.
    #[must_use]
    pub fn into_inner(self) -> async_graphql::BatchRequest {
        self.0
    }
}

#[async_trait::async_trait]
impl<S, R> FromRequest<S> for GraphQLBatchRequest<R>
where
    S: Send + Sync,
    R: IntoResponse + From<ParseRequestError>,
{
    type Rejection = R;

    async fn from_request(req: Request, _state: &S) -> Result<Self, Self::Rejection> {
        if let (&Method::GET, uri) = (req.method(), req.uri()) {
            let res = async_graphql::http::parse_query_string(uri.query().unwrap_or_default())
                .map_err(|err| {
                    ParseRequestError::Io(std::io::Error::new(
                        ErrorKind::Other,
                        format!("failed to parse graphql request from uri query: {}", err),
                    ))
                });
            Ok(Self(async_graphql::BatchRequest::Single(res?), PhantomData))
        } else {
            let content_type = req
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(ToString::to_string);
            let body_stream = req
                .into_body()
                .into_data_stream()
                .map_err(|err| std::io::Error::new(ErrorKind::Other, err.to_string()));
            let body_reader = tokio_util::io::StreamReader::new(body_stream).compat();
            Ok(Self(
                async_graphql::http::receive_batch_body(
                    content_type,
                    body_reader,
                    MultipartOptions::default(),
                )
                .await?,
                PhantomData,
            ))
        }
    }
}

/// Responder for a GraphQL response.
///
/// This contains a batch response, but since regular responses are a type of
/// batch response it works for both.
pub struct GraphQLResponse(pub async_graphql::BatchResponse);

impl From<async_graphql::Response> for GraphQLResponse {
    fn from(resp: async_graphql::Response) -> Self {
        Self(resp.into())
    }
}

impl From<async_graphql::BatchResponse> for GraphQLResponse {
    fn from(resp: async_graphql::BatchResponse) -> Self {
        Self(resp)
    }
}

impl IntoResponse for GraphQLResponse {
    fn into_response(self) -> Response {
        let body: Body = serde_json::to_string(&self.0).unwrap().into();
        let mut resp = Response::new(body);
        resp.headers_mut().insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        if self.0.is_ok() {
            if let Some(cache_control) = self.0.cache_control().value() {
                if let Ok(value) = HeaderValue::from_str(&cache_control) {
                    resp.headers_mut()
                        .insert(http::header::CACHE_CONTROL, value);
                }
            }
        }
        for h in self.0.http_headers() {
            if let Some(key) = h.0 {
                if let Ok(val_str) = h.1.to_str() {
                    if let Ok(k) = axum::http::HeaderName::from_str(key.as_str()) {
                        if let Ok(val) = axum::http::HeaderValue::from_str(val_str) {
                            resp.headers_mut().insert(k, val);
                        }
                    }
                }
            }
        }

        //resp.headers_mut().extend(self.0.http_headers().iter());
        resp
    }
}
