FROM rust:1.74.0-alpine3.18 as builder

WORKDIR /build

RUN apk add --no-cache musl-dev=1.2.4-r2

COPY .cargo .
COPY Cargo* ./

RUN mkdir -p src && \
    touch src/lib.rs && \
    cargo build --release --locked --target=x86_64-unknown-linux-musl

COPY src src/

RUN cargo build --release --locked --target=x86_64-unknown-linux-musl

FROM alpine:3.18.4

WORKDIR /app

RUN apk add --no-cache tini=0.19.0-r1

COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/pod-director .

ENTRYPOINT ["tini", "--"]
CMD ["./pod-director"]
