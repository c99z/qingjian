# syntax=docker/dockerfile:1
# 青简账号服务镜像。构建上下文是仓库根（workspace 要整个在），由 compose.yaml 指定；
# 排除规则在同目录的 Dockerfile.dockerignore。基础镜像可用构建参数换成内网镜像仓库。
ARG RUST_IMAGE=rust:1.96-slim-trixie
ARG RUNTIME_IMAGE=debian:trixie-slim

FROM ${RUST_IMAGE} AS build
# qingjian-platform 经 qingjian-predict 依赖 reqwest native-tls，链接期需要 OpenSSL。
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*
# 用镜像自带的工具链，不让 rust-toolchain.toml 触发 rustup 联网装 clippy / rustfmt。
ENV RUSTUP_TOOLCHAIN=${RUST_VERSION}
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p qingjian-account-server \
 && install -Dm755 target/release/qingjian-account-server /out/qingjian-account-server

FROM ${RUNTIME_IMAGE}
RUN apt-get update \
 && apt-get install -y --no-install-recommends libssl3t64 \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /data --shell /usr/sbin/nologin qingjian \
 && install -d -m 700 -o 10001 -g 10001 /data
COPY --from=build /out/qingjian-account-server /usr/local/bin/qingjian-account-server
ENV QINGJIAN_ACCOUNT_BIND=0.0.0.0:9080 \
    QINGJIAN_ACCOUNT_DB=/data/accounts.sqlite3
USER 10001:10001
EXPOSE 9080
HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
  CMD ["qingjian-account-server", "healthcheck"]
ENTRYPOINT ["qingjian-account-server"]
