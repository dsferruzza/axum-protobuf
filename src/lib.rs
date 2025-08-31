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
//! ## ProtoJson Extractor
//!
//! Additionally, this crate provides a [`ProtoJson`] extractor that can extract both protocol buffers and JSON payloads, depending upon the `content-type` header.
//!
//! Note that this does not implement [IntoResponse](https://docs.rs/axum/latest/axum/response/trait.IntoResponse.html) but you can use [`ProtoJson::infer_response`] to convert it into a JSON or protobuf response, based upon the `accept` header.
//! Otherwise, you can simply convert `ProtoJson` to `Json` or `Protobuf`.

// Force exposed items to be documented
#![deny(missing_docs)]

use axum::body::Body;
use axum::extract::FromRequest;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use prost::Message;

#[cfg(feature = "serde")]
mod protojson;

#[cfg(feature = "serde")]
pub use crate::protojson::*;

const PROTOBUF_CONTENT_TYPES: [&str; 3] = [
    "application/protobuf",
    "application/x-protobuf",
    "application/vnd.google.protobuf",
];
const PROTOBUF_CONTENT_TYPE: &str = PROTOBUF_CONTENT_TYPES[0];

/// Possible reasons why a request could be rejected.
pub enum ProtobufRejection {
    /// Decoding Protobuf failed.
    ProtobufDecodeError(prost::DecodeError),

    /// Buffering request body failed.
    FailedToBufferBody,

    /// Protobuf Content-Type header is missing.
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
/// ⚠️ Since parsing Protobuf requires consuming the request body, the [`Protobuf`] extractor must be
/// *last* if there are multiple extractors in a handler.
/// See ["the order of extractors"](https://docs.rs/axum/latest/axum/extract/index.html#the-order-of-extractors).
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
        req.headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .filter(|value| PROTOBUF_CONTENT_TYPES.contains(value))
            .ok_or(ProtobufRejection::MissingProtobufContentType)?;

        let mut body = req.into_body().into_data_stream();
        let mut buf = Vec::new();

        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|_| ProtobufRejection::FailedToBufferBody)?;
            buf.extend_from_slice(&chunk);
        }

        T::decode(buf.as_slice())
            .map(|x| Self(x))
            .map_err(ProtobufRejection::ProtobufDecodeError)
    }
}
