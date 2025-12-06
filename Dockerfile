FROM rust:1.91-slim-bookworm

WORKDIR /build

COPY Cargo.toml src ./

RUN cargo build --release
