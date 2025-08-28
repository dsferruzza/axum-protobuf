use axum::Json;
use axum::body::Body;
use axum::extract::FromRequest;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use prost::Message;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Protobuf, ProtobufRejection};

/// Possible reasons why a request could be rejected.
pub enum ProtoJsonRejection {
    ProtobufRejection(ProtobufRejection),
    JsonRejection(JsonRejection),
    MissingContentType,
}
impl IntoResponse for ProtoJsonRejection {
    fn into_response(self) -> Response {
        match self {
            ProtoJsonRejection::JsonRejection(rejection) => rejection.into_response(),
            ProtoJsonRejection::ProtobufRejection(rejection) => rejection.into_response(),
            ProtoJsonRejection::MissingContentType => {
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("Missing 'content-type' header that has the value `application/json` or `application/protobuf`"))
                    .unwrap() // we know this will be valid since we made it
            }
        }
    }
}

/// ProtoJson Extractor.
///
/// This can decode request bodies into some type that implements ([`prost::Message`] and [`Default`]) or [`serde::Deserialize`].
///
/// - The request doesn't have a `Content-Type: application/protobuf` / `Content-Type: application/json` (or similar) header.
/// - The request body failed to decode into the expected protobuf type.
/// - The body doesn't contain syntactically valid JSON.
/// - The body contains syntactically valid JSON, but it couldn't be deserialized into the target type.
/// - Buffering the request body fails.
///
/// ⚠️ Since parsing Protobuf and JSON requires consuming the request body, the [`ProtoJson`] extractor must be
/// *last* if there are multiple extractors in a handler.
/// See ["the order of extractors"](https://docs.rs/axum/latest/axum/extract/index.html#the-order-of-extractors).
pub struct ProtoJson<T>(pub T);

impl<T> ProtoJson<T>
where
    T: Message + Default + Serialize,
{
    /// Attempt to construct a response based on the `accept` header.
    pub fn try_infer_response(self, header_map: &HeaderMap) -> Option<Response> {
        let accept = header_map.get("accept").and_then(|v| v.to_str().ok());

        match accept {
            Some("application/json") => Some(Json(self.0).into_response()),
            Some("application/protobuf") => Some(Protobuf(self.0).into_response()),
            _ => None,
        }
    }

    /// Construct a response based on the `accept` header.
    ///
    /// If the `accept` header is not set or is not recognized, a [`StatusCode::BAD_REQUEST`] response is returned.
    pub fn infer_response(self, header_map: &HeaderMap) -> Response {
        self.try_infer_response(header_map).unwrap_or_else(
            || {
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::empty())
                    .unwrap()
            }, // we know this will be valid since we made it
        )
    }
}
impl<T> From<Json<T>> for ProtoJson<T> {
    fn from(x: Json<T>) -> ProtoJson<T> {
        ProtoJson(x.0)
    }
}
impl<T> From<ProtoJson<T>> for Json<T>
where
    T: DeserializeOwned,
{
    fn from(val: ProtoJson<T>) -> Self {
        Json(val.0)
    }
}
impl<T> From<Protobuf<T>> for ProtoJson<T> {
    fn from(x: Protobuf<T>) -> ProtoJson<T> {
        ProtoJson(x.0)
    }
}
impl<T> From<ProtoJson<T>> for Protobuf<T>
where
    T: Message + Default,
{
    fn from(val: ProtoJson<T>) -> Self {
        Protobuf(val.0)
    }
}

impl<S, T> FromRequest<S> for ProtoJson<T>
where
    T: Message + Default + DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ProtoJsonRejection;

    async fn from_request(
        req: axum::http::Request<Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let request_type = req
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok());

        match request_type {
            Some("application/json") => Json::<T>::from_request(req, state)
                .await
                .map(|x| x.into())
                .map_err(ProtoJsonRejection::JsonRejection),
            Some("application/protobuf") => Protobuf::<T>::from_request(req, state)
                .await
                .map(|x| x.into())
                .map_err(ProtoJsonRejection::ProtobufRejection),
            _ => Err(ProtoJsonRejection::MissingContentType),
        }
    }
}
