ARG KERNEL_SOURCE_URL=https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.18.54.tar.xz
ARG KERNEL_CHECKSUM=sha256:9df30b02dd8102bbd0be52556288ef6889ddbe7f1ddb96fbf847d0becf3eacac

FROM --platform=$BUILDPLATFORM debian:trixie@sha256:9cc080028c43b27d2074d63a5f9caf7166d731494965616c1a6d2827a004585c AS buildenv
RUN export DEBIAN_FRONTEND=noninteractive && apt-get update && apt-get install -y \
      build-essential squashfs-tools python3-yaml \
      patch diffutils sed mawk findutils zstd \
      python3 python3-packaging curl rsync cpio \
      flex bison pahole libssl-dev libelf-dev bc kmod && \
      rm -rf /var/lib/apt/lists/*
ARG BUILDPLATFORM
RUN if [ "${BUILDPLATFORM}" = "linux/amd64" ] || [ "${BUILDPLATFORM}" = "linux/x86_64" ]; then \
      apt-get update && apt-get install -y linux-headers-amd64 gcc-aarch64-linux-gnu && rm -rf /var/lib/apt/lists/*; fi
RUN if [ "${BUILDPLATFORM}" = "linux/arm64" ] || [ "${BUILDPLATFORM}" = "linux/aarch64" ]; then \
      apt-get update && apt-get install -y linux-headers-arm64 gcc-x86-64-linux-gnu && rm -rf /var/lib/apt/lists/*; fi
RUN useradd -ms /bin/sh build
COPY --chown=build:build docker-build.sh /build/docker-build.sh
USER build
WORKDIR /build

FROM scratch AS source
ARG KERNEL_SOURCE_URL
ARG KERNEL_CHECKSUM
ADD --checksum=${KERNEL_CHECKSUM} ${KERNEL_SOURCE_URL} /src.tar.xz

FROM --platform=$BUILDPLATFORM buildenv AS build
COPY --from=source --chown=build:build /src.tar.xz /build/src.tar.xz
RUN mkdir /build/src && tar -C /build/src --strip-components=1 -xf /build/src.tar.xz && rm /build/src.tar.xz
ARG BUILDPLATFORM
ARG TARGETPLATFORM
ENV BUILDPLATFORM=${BUILDPLATFORM}
ENV TARGETPLATFORM=${TARGETPLATFORM}
WORKDIR /build/src
RUN /build/docker-build.sh
COPY --chown=build:build docker-install.sh /build/docker-install.sh
RUN /build/docker-install.sh

FROM scratch AS final
COPY --from=build /build/src/kernel.image /kernel.efi
COPY --from=build /build/src/kernel.modules.tgz /kernel.modules.tgz
