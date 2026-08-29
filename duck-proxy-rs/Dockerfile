FROM rust:bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/duck-proxy-rs /app/duck-proxy-rs
COPY --from=builder /app/config.yaml /app/config.yaml

EXPOSE 8080
ENV RUST_LOG=duck_proxy_rs=info,tower_http=info
ENTRYPOINT ["/app/duck-proxy-rs"]
