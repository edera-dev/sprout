# syntax=docker/dockerfile:1.7-labs
ARG RUST_PROFILE=release
ARG RUST_TARGET_SUBDIR=release

FROM --platform=$BUILDPLATFORM rustlang/rust:nightly-alpine@sha256:b8107fa66d3e5ad7f729d3347c7feedbd3f4b60b01006edce39eb6b994ff00bd AS build
RUN apk --no-cache add musl-dev busybox-static
ARG RUST_PROFILE
RUN adduser -S -s /bin/sh build
COPY \
    --exclude=rust-toolchain.toml \
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
