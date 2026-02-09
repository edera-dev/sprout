# syntax=docker/dockerfile:1.7-labs
ARG RUST_PROFILE=release
ARG RUST_TARGET_SUBDIR=release

FROM --platform=$BUILDPLATFORM rust:1.93.0-alpine@sha256:69d7b9d9aeaf108a1419d9a7fcf7860dcc043e9dbd1ab7ce88e44228774d99e9 AS build
RUN apk --no-cache add musl-dev busybox-static
ARG RUST_PROFILE
RUN adduser -S -s /bin/sh build
COPY \
    --exclude=rust-toolchain.toml \
    --exclude=hack \
    --chown=build:build \
    . /build
WORKDIR /build
ARG TARGETPLATFORM
ARG RUST_TARGET_SUBDIR
RUN if [ "${TARGETPLATFORM}" = "linux/amd64" ] || [ "${TARGETPLATFORM}" = "linux/x86_64" ]; then \
      rustup target add x86_64-unknown-uefi; cargo build --bin sprout --profile "${RUST_PROFILE}" --target x86_64-unknown-uefi && \
      cp "target/x86_64-unknown-uefi/${RUST_TARGET_SUBDIR}/sprout.efi" /sprout.efi; fi
RUN if [ "${TARGETPLATFORM}" = "linux/arm64" ] || [ "${TARGETPLATFORM}" = "linux/aarch64" ]; then \
      rustup target add aarch64-unknown-uefi; cargo build --bin sprout --profile "${RUST_PROFILE}" --target aarch64-unknown-uefi && \
      cp "target/aarch64-unknown-uefi/${RUST_TARGET_SUBDIR}/sprout.efi" /sprout.efi; fi

FROM scratch AS final
COPY --from=build /sprout.efi /sprout.efi
