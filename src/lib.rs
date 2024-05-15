use async_trait::async_trait;
use axum::{response::{IntoResponse, Response}, http::StatusCode, body::Body, extract::FromRequest};
use futures_util::StreamExt;

#[cfg(feature = "serde")]
mod protojson;
#[cfg(feature = "serde")]
pub use protojson::*;

/// Possible reasons why a request could be rejected.
pub enum ProtobufRejection {
    ProtobufDecodeError(prost::DecodeError),
    FailedToBufferBody,
    MissingProtobufContentType,
}
impl IntoResponse for ProtobufRejection {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            ProtobufRejection::ProtobufDecodeError(_) => (
                StatusCode::BAD_REQUEST,
                "Protobuf decoding error",
            ),
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
/// See ["the order of extractors"][https://docs.rs/axum/latest/axum/extract/index.html#the-order-of-extractors]
pub struct Protobuf<T>(pub T);

impl<T> IntoResponse for Protobuf<T>
where
    T: prost::Message + Default,
{
    fn into_response(self) -> Response {
        let mut buf = Vec::new();
        if let Err(e) = self.0.encode(&mut buf) {
            buf = format!("protobuf encoding error: {}", e).into_bytes();

            return Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(buf))
                .unwrap() // we know this will be valid since we made it
        }

        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/protobuf")
            .body(Body::from(buf))   
            .unwrap() // we know this will be valid since we made it
    }
}
#[async_trait]
impl<S, T> FromRequest<S> for Protobuf<T>
where
    T: prost::Message + Default, 
    S: Send + Sync,   
{
    type Rejection = ProtobufRejection;

    async fn from_request(req: axum::http::Request<Body>, _: &S) -> Result<Self, Self::Rejection> {
        req
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .filter(|value| *value == "application/protobuf")
            .ok_or(ProtobufRejection::MissingProtobufContentType)?;

        let mut body = req.into_body().into_data_stream();
        let mut buf = Vec::new();

        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|_| ProtobufRejection::FailedToBufferBody)?;
            buf.extend_from_slice(&chunk);
        }

        T::decode(buf.as_slice())
            .map(|x| Self(x))
            .map_err(|e| ProtobufRejection::ProtobufDecodeError(e))
    }
}