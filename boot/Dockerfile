FROM --platform=$BUILDPLATFORM debian:trixie@sha256:fd8f5a1df07b5195613e4b9a0b6a947d3772a151b81975db27d47f093f60c6e6 AS build
ARG BUILDPLATFORM
ARG EFI_NAME
RUN export DEBIAN_FRONTEND=noninteractive && apt-get update && apt-get install -y \
      parted dosfstools mtools && \
      rm -rf /var/lib/apt/lists/*
WORKDIR /work
COPY sprout.efi /work/${EFI_NAME}.EFI
COPY sprout.toml /work/SPROUT.TOML
COPY kernel.efi /work/KERNEL.EFI
COPY shell.efi /work/SHELL.EFI
RUN truncate -s256MiB sprout.img && \
    parted --script sprout.img mklabel gpt > /dev/null 2>&1 && \
    parted --script sprout.img mkpart primary fat32 1MiB 100% > /dev/null 2>&1 && \
    parted --script sprout.img set 1 esp on > /dev/null 2>&1 && \
    mkfs.vfat -F32 -n EFI sprout.img && \
    mmd -i sprout.img ::/EFI && \
    mmd -i sprout.img ::/EFI/BOOT && \
    mcopy -i sprout.img ${EFI_NAME}.EFI ::/EFI/BOOT/ && \
    mcopy -i sprout.img KERNEL.EFI ::/EFI/BOOT/ && \
    mcopy -i sprout.img SHELL.EFI ::/EFI/BOOT/ && \
    mcopy -i sprout.img SPROUT.TOML ::/ && \
    mv sprout.img /sprout.img

FROM scratch AS final
COPY --from=build /sprout.img /sprout.img
