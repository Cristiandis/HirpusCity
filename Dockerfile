FROM rust:1-alpine AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN apk add --no-cache musl-dev && cargo build --release

FROM scratch
COPY --from=build /app/target/release/hirpuscity /hirpuscity
COPY web /web
ENV HIRPUS_LISTEN=0.0.0.0:8080 \
    HIRPUS_DATA_DIR=/data \
    HIRPUS_ASSETS_DIR=/web
EXPOSE 8080
ENTRYPOINT ["/hirpuscity"]
