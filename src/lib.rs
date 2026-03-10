//! # axum-protobuf
//!
//! A crate bringing [protocol buffers](https://protobuf.dev/) to [axum](https://github.com/tokio-rs/axum) via [prost](https://github.com/tokio-rs/prost).
//!
//! ## Protobuf Extractor / Response
//!
//! This crate provides a [`Protobuf`] extractor and response.
//! The usage is very similar to axum's [Json](https://docs.rs/axum/latest/axum/struct.Json.html).
//! Refer to their documentation for usage, but replace `Json` with `Protobuf`.
//!
//! The only difference is that `T` must implement [prost::Message](https://docs.rs/prost/latest/prost/trait.Message.html).
//!
//! ## ProtoJson Extractor / Response
//!
//! Additionally, this crate provides a [`ProtoJson`] extractor that can extract both protocol buffers and JSON payloads, depending upon the `content-type` header.
//!
//! `ProtoJson` implements [IntoResponse](https://docs.rs/axum/latest/axum/response/trait.IntoResponse.html) and automatically negotiates the response format based on the `accept` header (using RFC 7231 content negotiation).
//! When no `accept` header is present or no supported format matches, it defaults to JSON.
//!
//! You can also convert `ProtoJson` to `Json` or `Protobuf` directly.

// Force exposed items to be documented
#![deny(missing_docs)]

use axum::body::{Body, to_bytes};
use axum::extract::FromRequest;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use prost::Message;

#[cfg(feature = "serde")]
mod protojson;

#[cfg(feature = "serde")]
pub use crate::protojson::*;

const PROTOBUF_CONTENT_TYPE: &str = "application/protobuf";

/// Check if a Content-Type header value matches any protobuf MIME type.
/// Ignores parameters (charset, etc.) — only checks type/subtype.
fn is_protobuf_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|ct| ct.to_str().ok())
        .and_then(|ct| ct.parse::<mime::Mime>().ok())
        .is_some_and(|mime| {
            mime.type_() == "application"
                && matches!(
                    mime.subtype().as_str(),
                    "protobuf" | "x-protobuf" | "vnd.google.protobuf"
                )
        })
}

/// Check if a Content-Type header value matches JSON MIME type.
/// Ignores parameters (charset, etc.) — only checks type/subtype.
#[cfg(feature = "serde")]
fn is_json_content_type(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|ct| ct.to_str().ok())
        .and_then(|ct| ct.parse::<mime::Mime>().ok())
        .is_some_and(|mime| {
            mime.type_() == "application"
                && (mime.subtype() == "json" || mime.suffix().is_some_and(|s| s == "json"))
        })
}

/// Possible reasons why a request could be rejected.
#[derive(Debug, thiserror::Error)]
pub enum ProtobufRejection {
    /// Decoding Protobuf failed.
    #[error("Protobuf decoding error")]
    ProtobufDecodeError(#[from] prost::DecodeError),

    /// Buffering request body failed.
    #[error("Error reading request body")]
    FailedToBufferBody,

    /// Protobuf Content-Type header is missing.
    #[error("Missing 'content-type: application/protobuf' header")]
    MissingProtobufContentType,
}

impl IntoResponse for ProtobufRejection {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ProtobufRejection::ProtobufDecodeError(_) => {
                (StatusCode::BAD_REQUEST, "Protobuf decoding error")
            }
            ProtobufRejection::FailedToBufferBody => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error reading request body",
            ),
            ProtobufRejection::MissingProtobufContentType => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "Missing 'content-type: application/protobuf' header",
            ),
        };

        Response::builder()
            .status(status)
            .body(Body::from(body))
            .unwrap() // we know this will be valid since we made it
    }
}

/// Protobuf Extractor / Response.
///
/// When used as an extractor, it can decode request bodies into some type that implements [`prost::Message`] and [`Default`].
///
/// The request will be rejected (and a [`ProtobufRejection`] will be returned) if:
/// - The request doesn't have a `Content-Type: application/protobuf` (or similar) header.
/// - The request body failed to decode into the expected protobuf type.
/// - Buffering the request body fails.
///
/// # Body size limit
///
/// This extractor relies on axum's [`DefaultBodyLimit`](https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html)
/// (2 MiB by default) to cap request body size. If you have disabled it, apply
/// [`RequestBodyLimitLayer`](https://docs.rs/tower-http/latest/tower_http/limit/struct.RequestBodyLimitLayer.html)
/// to prevent unbounded memory consumption.
///
/// ⚠️ Since parsing Protobuf requires consuming the request body, the [`Protobuf`] extractor must be
/// *last* if there are multiple extractors in a handler.
/// See ["the order of extractors"](https://docs.rs/axum/latest/axum/extract/index.html#the-order-of-extractors).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Protobuf<T>(pub T);

impl<T> IntoResponse for Protobuf<T>
where
    T: Message + Default,
{
    fn into_response(self) -> Response {
        let mut buf = Vec::new();

        if let Err(e) = self.0.encode(&mut buf) {
            buf = format!("protobuf encoding error: {e}").into_bytes();

            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(buf))
                .unwrap() // we know this will be valid since we made it
        } else {
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, PROTOBUF_CONTENT_TYPE)
                .body(Body::from(buf))
                .unwrap() // we know this will be valid since we made it
        }
    }
}
impl<S, T> FromRequest<S> for Protobuf<T>
where
    T: Message + Default,
    S: Send + Sync,
{
    type Rejection = ProtobufRejection;

    async fn from_request(req: axum::http::Request<Body>, _: &S) -> Result<Self, Self::Rejection> {
        if !is_protobuf_content_type(req.headers()) {
            Err(ProtobufRejection::MissingProtobufContentType)
        } else {
            let bytes = to_bytes(req.into_body(), usize::MAX)
                .await
                .map_err(|_| ProtobufRejection::FailedToBufferBody)?;

            T::decode(bytes)
                .map(Self)
                .map_err(ProtobufRejection::ProtobufDecodeError)
        }
    }
}
