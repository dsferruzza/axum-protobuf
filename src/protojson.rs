use axum::Json;
use axum::body::Body;
use axum::extract::FromRequest;
use axum::extract::Request;
use axum::extract::rejection::JsonRejection;
use axum::http::HeaderName;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::ACCEPT;
use axum::response::{IntoResponse, Response};
use axum_extra::headers::Header;
use prost::Message;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::iter::once;
use std::ops::{Deref, DerefMut};

use crate::{Protobuf, ProtobufRejection, is_json_content_type, is_protobuf_content_type};

/// Possible reasons why a request could be rejected.
#[derive(Debug, thiserror::Error)]
pub enum ProtoJsonRejection {
    /// Protobuf-related error.
    #[error(transparent)]
    ProtobufRejection(#[from] ProtobufRejection),

    /// JSON-related error.
    #[error(transparent)]
    JsonRejection(#[from] JsonRejection),

    /// Content-Type header is missing or has an unsupported value.
    #[error(
        "Missing 'content-type' header that has the value 'application/json' or 'application/protobuf'"
    )]
    MissingContentType,
}
impl IntoResponse for ProtoJsonRejection {
    fn into_response(self) -> Response {
        match self {
            ProtoJsonRejection::JsonRejection(rejection) => rejection.into_response(),
            ProtoJsonRejection::ProtobufRejection(rejection) => rejection.into_response(),
            ProtoJsonRejection::MissingContentType => {
                Response::builder()
                    .status(StatusCode::UNSUPPORTED_MEDIA_TYPE)
                    .body(Body::from("Missing 'content-type' header that has the value 'application/json' or 'application/protobuf'"))
                    .unwrap() // we know this will be valid since we made it
            }
        }
    }
}

/// Response format determined by content negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResponseFormat {
    Json,
    Protobuf,
}

/// An optional `Accept` header extractor.
pub struct OptionalAcceptHeader(
    /// The raw `Accept` header value if found.
    pub Option<HeaderValue>,
);

impl Header for OptionalAcceptHeader {
    fn name() -> &'static HeaderName {
        &ACCEPT
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, axum_extra::headers::Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i HeaderValue>,
    {
        Ok(Self(values.next().cloned()))
    }

    fn encode<E: Extend<HeaderValue>>(&self, values: &mut E) {
        if let Some(hv) = &self.0 {
            values.extend(once(hv.to_owned()));
        }
    }
}

/// Negotiate the response format from an Accept header value using RFC 7231 content negotiation.
///
/// Parses comma-separated media ranges with optional quality values, sorts by quality descending,
/// and returns the first supported format. Handles wildcards (`*/*`, `application/*`).
/// Defaults to JSON when Accept is missing, unparseable, or no supported format matches.
fn negotiate_format(accept: Option<&HeaderValue>) -> ResponseFormat {
    accept
        .and_then(|a| a.to_str().ok())
        .and_then(|accept_str| {
            let mut candidates: Vec<(mime::Mime, f32)> = accept_str
                .split(',')
                .filter_map(|part| {
                    let mime = part.trim().parse::<mime::Mime>().ok()?;
                    let q = mime
                        .get_param("q")
                        .and_then(|v| v.as_str().parse::<f32>().ok())
                        .map(|q| q.clamp(0.0, 1.0))
                        .unwrap_or(1.0);
                    Some((mime, q))
                })
                .collect();

            // Sort by quality descending (stable sort preserves order for equal quality)
            candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            candidates.iter().find_map(|(mime, _)| {
                if (mime.type_() == mime::STAR && mime.subtype() == mime::STAR)
                    || (mime.type_() == "application" && mime.subtype() == mime::STAR)
                {
                    Some(ResponseFormat::Json)
                } else if mime.type_() == "application" {
                    match mime.subtype().as_str() {
                        "json" => Some(ResponseFormat::Json),
                        "protobuf" | "x-protobuf" | "vnd.google.protobuf" => {
                            Some(ResponseFormat::Protobuf)
                        }
                        _ => None,
                    }
                } else {
                    None
                }
            })
        })
        .unwrap_or(ResponseFormat::Json)
}

/// ProtoJson Extractor / Response.
///
/// This can decode request bodies into some type that implements ([`prost::Message`] and [`Default`]) or [`serde::Deserialize`],
/// depending on the `content-type` header.
///
/// The request will be rejected (and a [`ProtoJsonRejection`] will be returned) if:
/// - The request doesn't have a `Content-Type: application/protobuf` / `Content-Type: application/json` (or similar) header.
/// - The request body failed to decode into the expected protobuf type.
/// - The body doesn't contain syntactically valid JSON.
/// - The body contains syntactically valid JSON, but it couldn't be deserialized into the target type.
/// - Buffering the request body fails.
///
/// When used as a response, it implements [`IntoResponse`] and automatically negotiates the response
/// format based on the `accept` header using RFC 7231 content negotiation. When no `accept` header
/// is present or no supported format matches, it defaults to JSON.
///
/// When extracted from a request, the `accept` header is captured automatically.
/// For manual construction (e.g., in GET handlers), use [`ProtoJson::with_accept`] to specify the
/// `accept` header, or [`ProtoJson::new`] to default to JSON.
///
/// # Body size limit
///
/// This extractor relies on axum's [`DefaultBodyLimit`](https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html)
/// (2 MiB by default) to cap request body size. If you have disabled it, apply
/// [`RequestBodyLimitLayer`](https://docs.rs/tower-http/latest/tower_http/limit/struct.RequestBodyLimitLayer.html)
/// to prevent unbounded memory consumption.
///
/// ⚠️ Since parsing Protobuf and JSON requires consuming the request body, the [`ProtoJson`] extractor must be
/// *last* if there are multiple extractors in a handler.
/// See ["the order of extractors"](https://docs.rs/axum/latest/axum/extract/index.html#the-order-of-extractors).
pub struct ProtoJson<T> {
    inner: T,
    accept: Option<HeaderValue>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for ProtoJson<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtoJson")
            .field("inner", &self.inner)
            .field("accept", &self.accept)
            .finish()
    }
}

impl<T: Clone> Clone for ProtoJson<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            accept: self.accept.clone(),
        }
    }
}

impl<T> ProtoJson<T> {
    /// Create a new `ProtoJson` with no Accept header (defaults to JSON on response).
    pub fn new(value: T) -> Self {
        Self {
            inner: value,
            accept: None,
        }
    }

    /// Create a new `ProtoJson` with a specific Accept header value.
    ///
    /// Use this in handlers that construct responses manually (e.g., GET handlers)
    /// to enable content negotiation based on the client's `accept` header.
    pub fn with_accept(accept: Option<HeaderValue>, value: T) -> Self {
        Self {
            inner: value,
            accept,
        }
    }

    /// Consume the `ProtoJson` and return the inner value.
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> Deref for ProtoJson<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T> DerefMut for ProtoJson<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> IntoResponse for ProtoJson<T>
where
    T: Message + Default + Serialize,
{
    fn into_response(self) -> Response {
        match negotiate_format(self.accept.as_ref()) {
            ResponseFormat::Json => Json(self.inner).into_response(),
            ResponseFormat::Protobuf => Protobuf(self.inner).into_response(),
        }
    }
}

impl<T> From<Json<T>> for ProtoJson<T> {
    fn from(x: Json<T>) -> ProtoJson<T> {
        ProtoJson::new(x.0)
    }
}
impl<T> From<ProtoJson<T>> for Json<T>
where
    T: DeserializeOwned,
{
    fn from(val: ProtoJson<T>) -> Self {
        Json(val.inner)
    }
}
impl<T> From<Protobuf<T>> for ProtoJson<T> {
    fn from(x: Protobuf<T>) -> ProtoJson<T> {
        ProtoJson::new(x.0)
    }
}
impl<T> From<ProtoJson<T>> for Protobuf<T>
where
    T: Message + Default,
{
    fn from(val: ProtoJson<T>) -> Self {
        Protobuf(val.inner)
    }
}

impl<S, T> FromRequest<S> for ProtoJson<T>
where
    T: Message + Default + DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ProtoJsonRejection;

    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let accept = req.headers().get(ACCEPT).cloned();

        if is_protobuf_content_type(req.headers()) {
            let protobuf = Protobuf::<T>::from_request(req, state)
                .await
                .map_err(ProtoJsonRejection::ProtobufRejection)?;
            Ok(ProtoJson {
                inner: protobuf.0,
                accept,
            })
        } else if is_json_content_type(req.headers()) {
            let json = Json::<T>::from_request(req, state)
                .await
                .map_err(ProtoJsonRejection::JsonRejection)?;
            Ok(ProtoJson {
                inner: json.0,
                accept,
            })
        } else {
            Err(ProtoJsonRejection::MissingContentType)
        }
    }
}
