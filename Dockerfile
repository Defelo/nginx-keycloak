FROM rust:alpine AS builder

WORKDIR /build

RUN apk add --no-cache musl-dev upx

COPY Cargo.toml .
COPY Cargo.lock .

RUN echo "fn main() {}" > tmp.rs \
    && sed -i 's#src/main.rs#tmp.rs#' Cargo.toml \
    && cargo build --release \
    && sed -i 's#tmp.rs#src/main.rs#' Cargo.toml \
    && rm tmp.rs

COPY src src

RUN cargo build --release \
    && upx --best --lzma -o app target/release/nginx-keycloak


FROM scratch

LABEL org.opencontainers.image.source="https://github.com/Defelo/nginx-keycloak"

ENV ROCKET_PROFILE="release" \
    ROCKET_ADDRESS=0.0.0.0 \
    ROCKET_PORT=80

EXPOSE 80

COPY --from=builder /build/app /

ENTRYPOINT ["/app"]
