FROM rust:1.91-slim-bookworm

WORKDIR /build

COPY Cargo.toml ./
COPY src ./src

RUN cargo build --release
