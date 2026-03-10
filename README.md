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

### ProtoJson Extractor / Response

Additionally, this crate provides a `ProtoJson` extractor that can extract both protocol buffers and JSON payloads, depending upon the `content-type` header.

`ProtoJson` implements [IntoResponse](https://docs.rs/axum/latest/axum/response/trait.IntoResponse.html) and automatically negotiates the response format based on the `accept` header using RFC 7231 content negotiation. When no `accept` header is present or no supported format matches, it defaults to JSON.

You can also convert `ProtoJson` to `Json` or `Protobuf` directly.

## License

MIT License Copyright (c) 2025 David Sferruzza
