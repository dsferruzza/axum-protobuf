use axum::Router;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum_protobuf::{ProtoJson, Protobuf};
use prost::Message;
use serde::{Deserialize, Serialize};

pub fn build_app() -> Router {
    Router::new()
        .route("/protobuf/input", post(protobuf_input_handler))
        .route("/protobuf/output", get(protobuf_output_handler))
        .route("/protojson/input", post(protojson_input_handler))
        .route("/protojson/output", get(protojson_output_handler))
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Message)]
pub struct TestMessage {
    #[prost(string, tag = "1")]
    pub test: String,
}

#[axum::debug_handler]
pub async fn protobuf_input_handler(Protobuf(input): Protobuf<TestMessage>) -> String {
    input.test
}

#[axum::debug_handler]
pub async fn protobuf_output_handler() -> Protobuf<TestMessage> {
    Protobuf(TestMessage {
        test: "test".to_owned(),
    })
}

#[axum::debug_handler]
pub async fn protojson_input_handler(ProtoJson(input): ProtoJson<TestMessage>) -> String {
    input.test
}

#[axum::debug_handler]
pub async fn protojson_output_handler(headers: HeaderMap) -> Response {
    ProtoJson(TestMessage {
        test: "test".to_owned(),
    })
    .infer_response(&headers)
}
