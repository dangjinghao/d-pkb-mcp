FROM rust:1-trixie AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --locked --release

FROM debian:trixie-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates ripgrep \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/d-pkb-mcp /usr/local/bin/d-pkb-mcp

EXPOSE 8000
ENTRYPOINT ["/usr/local/bin/d-pkb-mcp"]
CMD ["--pkb-root", "/data/D", "--tmp-path", "/data/.tmp", "--addr", "0.0.0.0:8000"]
