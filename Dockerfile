FROM docker.io/library/rust:alpine as rust-base

RUN apk add --no-cache protobuf musl-dev protobuf-c protobuf-c-compiler


FROM rust-base AS await-factory

RUN cargo install --git='https://github.com/linkerd/linkerd-await.git' linkerd-await


FROM rust-base AS crate-artifactory

WORKDIR /crate

COPY . /crate

RUN cargo build --release


FROM gcr.io/distroless/static-debian12 as release

COPY --from=await-factory /usr/local/cargo/bin/linkerd-await /bin/linkerd-await
COPY --from=crate-artifactory /crate/target/release/grpcroute-demo /bin/grpcroute-demo

ENTRYPOINT ["/bin/linkerd-await", "--shutdown", "--"]
CMD  ["/bin/grpcroute-demo"]
