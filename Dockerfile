# syntax=docker/dockerfile:1.7

ARG RUST_VERSION=1.85
ARG NODE_VERSION=22
ARG TRUNK_VERSION=0.21.14
ARG SQLX_CLI_VERSION=0.8.6

FROM node:${NODE_VERSION}-bookworm-slim AS css-builder

WORKDIR /src

COPY package.json package-lock.json ./
RUN npm ci --ignore-scripts

COPY frontend/app.css frontend/app.css
COPY frontend/index.html frontend/index.html
COPY src ./src

RUN npm run build:css

FROM rust:${RUST_VERSION}-bookworm AS rust-builder

ARG TRUNK_VERSION
ARG SQLX_CLI_VERSION

WORKDIR /src

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --version "${TRUNK_VERSION}" --locked \
    && cargo install sqlx-cli --version "${SQLX_CLI_VERSION}" \
        --no-default-features \
        --features postgres,rustls \
        --locked

COPY Cargo.toml Cargo.lock Trunk.toml ./
COPY src ./src
COPY frontend/index.html frontend/index.html
COPY frontend/app.css frontend/app.css
COPY --from=css-builder /src/frontend/generated.css frontend/generated.css
COPY migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --bin xiaoluoquiz-server --features server

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    trunk build --release --dist dist

FROM debian:trixie-slim AS runtime

RUN apt-get update \
    && apt-get install --no-install-recommends --yes ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 10001 xiaoluoquiz \
    && useradd --system --uid 10001 --gid 10001 --home-dir /app \
        --no-create-home --shell /usr/sbin/nologin xiaoluoquiz

WORKDIR /app

ENV APP_HOST=0.0.0.0 \
    APP_PORT=8000 \
    STATIC_DIR=/app/dist

COPY --from=rust-builder --chown=xiaoluoquiz:xiaoluoquiz \
    /src/target/release/xiaoluoquiz-server /usr/local/bin/xiaoluoquiz-server
COPY --from=rust-builder --chown=xiaoluoquiz:xiaoluoquiz \
    /usr/local/cargo/bin/sqlx /usr/local/bin/sqlx
COPY --from=rust-builder --chown=xiaoluoquiz:xiaoluoquiz \
    /src/dist /app/dist
COPY --from=rust-builder --chown=xiaoluoquiz:xiaoluoquiz \
    /src/migrations /app/migrations

USER xiaoluoquiz

EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=5 \
    CMD ["curl", "--fail", "--silent", "--show-error", "http://127.0.0.1:8000/api/v1/health"]

ENTRYPOINT ["/usr/local/bin/xiaoluoquiz-server"]
