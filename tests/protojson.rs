mod app;

use app::build_app;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode};
use prost::Message;
use serde_json::{from_slice, json};
use tower::ServiceExt;

use crate::app::TestMessage;

#[tokio::test]
async fn protojson_extractor_no_content_type() {
    let app = build_app();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protojson/input")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    assert_eq!(
        body.iter().as_slice(),
        b"Missing 'content-type' header that has the value 'application/json' or 'application/protobuf'"
    );
}

#[tokio::test]
async fn protojson_extractor_protobuf_empty_body() {
    let app = build_app();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protojson/input")
                .header("Content-Type", "application/protobuf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    assert_eq!(body.iter().as_slice(), b"");
}

#[tokio::test]
async fn protojson_extractor_protobuf_invalid_body() {
    let app = build_app();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protojson/input")
                .header("Content-Type", "application/protobuf")
                .body(Body::from(b"invalid".as_slice()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    assert_eq!(body.iter().as_slice(), b"Protobuf decoding error");
}

#[tokio::test]
async fn protojson_extractor_json_empty_body() {
    let app = build_app();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protojson/input")
                .header("Content-Type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    assert_eq!(
        body.iter().as_slice(),
        b"Failed to parse the request body as JSON: EOF while parsing a value at line 1 column 0"
    );
}

#[tokio::test]
async fn protojson_extractor_protobuf_simple() {
    let app = build_app();
    let test_string = "test";
    let mut input = Vec::new();
    TestMessage {
        test: test_string.to_owned(),
    }
    .encode(&mut input)
    .unwrap();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protojson/input")
                .header("Content-Type", "application/protobuf")
                .body(Body::from(input))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    assert_eq!(body.iter().as_slice(), test_string.as_bytes());
}

#[tokio::test]
async fn protojson_extractor_protobuf_alternative_content_types() {
    let app = build_app();
    let test_string = "test";
    let mut input = Vec::new();
    TestMessage {
        test: test_string.to_owned(),
    }
    .encode(&mut input)
    .unwrap();
    let content_types = [
        "application/protobuf",
        "application/x-protobuf",
        "application/vnd.google.protobuf",
    ];

    for ct in content_types {
        println!("Trying Content-Type: {ct}");
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/protojson/input")
                    .header("Content-Type", ct)
                    .body(Body::from(input.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
        dbg!(&body);
        assert_eq!(body.iter().as_slice(), test_string.as_bytes());
    }
}

#[tokio::test]
async fn protojson_extractor_json_simple() {
    let app = build_app();
    let test_string = "test";
    let input = json!({ "test": test_string }).to_string();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/protojson/input")
                .header("Content-Type", "application/json")
                .body(Body::from(input))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    assert_eq!(body.iter().as_slice(), test_string.as_bytes());
}

#[tokio::test]
async fn protojson_response_no_accept() {
    let app = build_app();
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/protojson/output")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    assert_eq!(
        body,
        "Missing 'accept' header with value 'application/json' or 'application/protobuf'"
    );
}

#[tokio::test]
async fn protojson_response_protobuf() {
    let app = build_app();
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/protojson/output")
                .header("Accept", "application/protobuf")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get("Content-Type").unwrap(),
        "application/protobuf"
    );
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    let message = TestMessage::decode(body).unwrap();
    assert_eq!(message.test, "test");
}

#[tokio::test]
async fn protojson_response_json() {
    let app = build_app();
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/protojson/output")
                .header("Accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers().get("Content-Type").unwrap(),
        "application/json"
    );
    let body = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    dbg!(&body);
    let message = from_slice::<TestMessage>(body.iter().as_slice()).unwrap();
    assert_eq!(message.test, "test");
}
