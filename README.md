# axum-protobuf

[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![CI](https://github.com/dsferruzza/axum-protobuf/workflows/CI/badge.svg)
[![Crates.io Version](https://img.shields.io/crates/v/axum-protobuf.svg)](https://crates.io/crates/axum-protobuf)
[![Documentation](https://docs.rs/axum-protobuf/badge.svg)](https://docs.rs/axum-protobuf)

A crate bringing [protocol buffers](https://protobuf.dev/) to [axum](https://github.com/tokio-rs/axum) via [prost](https://github.com/tokio-rs/prost).

## Features

### Protobuf Extractor / Response

This crate provides a `Protobuf` extractor and response.
The usage is very similar to axum's [Json](https://docs.rs/axum/latest/axum/struct.Json.html).
Refer to their documentation for usage, but replace `Json` with `Protobuf`.

The only difference is that `T` must implement [prost::Message](https://docs.rs/prost/latest/prost/trait.Message.html).

### ProtoJson Extractor

Additionally, this crate provides a `ProtoJson` extractor that can extract both protocol buffers and JSON payloads, depending upon the `content-type` header.

Note that this does not implement [IntoResponse](https://docs.rs/axum/latest/axum/response/trait.IntoResponse.html) but you can use `infer_response` to convert it into a JSON or protobuf response, based upon the `accept` header.
Otherwise, you can simply convert `ProtoJson` to `Json` or `Protobuf`.

## License

MIT License Copyright (c) 2025 David Sferruzza
