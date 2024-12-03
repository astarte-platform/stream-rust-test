# Copyright 2024 SECO Mind Srl
#
# SPDX-License-Identifier: Apache-2.0

FROM rust:1.81-alpine3.20 AS build

RUN mkdir /stream-rust-test && apk update && apk upgrade && \
    apk add --no-cache alpine-sdk musl-dev pkgconfig openssl-dev openssl-libs-static sqlite-static sqlite-dev

COPY . /stream-rust-test

WORKDIR /stream-rust-test

RUN cargo build --release

FROM alpine:3.20

COPY scripts/docker/entrypoint.sh /entrypoint.sh

COPY --from=build /stream-rust-test/target/release/stream-rust-test /usr/bin/

RUN mkdir -p /tmp/stream-rust-test/store/

CMD ["/entrypoint.sh"]
