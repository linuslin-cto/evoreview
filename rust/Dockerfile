# 仅构建 Rust 二进制（无前端）。仓库根目录上下文：
#   docker build -f rust/Dockerfile -t xiezhi-server:dev .
#
# 完整 Web+API 镜像请用仓库根 Dockerfile.rust。

FROM rust:1.85-bookworm AS builder
WORKDIR /src
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY rust/Cargo.toml rust/Cargo.lock ./
COPY rust/crates ./crates
RUN cargo build --release -p xiezhi-server -p xiezhi-cli \
    && strip target/release/xiezhi-server target/release/xiezhi

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates git ripgrep \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/target/release/xiezhi-server /usr/local/bin/xiezhi-server
COPY --from=builder /src/target/release/xiezhi /usr/local/bin/xiezhi
COPY conf/prompt_templates.yml /app/conf/prompt_templates.yml
COPY conf/scr /app/conf/scr
ENV SERVER_PORT=5001
ENV RUST_LOG=info
EXPOSE 5001
CMD ["xiezhi-server"]
