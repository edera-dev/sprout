FROM --platform=$BUILDPLATFORM debian:trixie@sha256:2c91e484d93f0830a7e05a2b9d92a7b102be7cab562198b984a84fdbc7806d91 AS build
ARG BUILDPLATFORM
ARG EFI_NAME
RUN export DEBIAN_FRONTEND=noninteractive && apt-get update && apt-get install -y \
      parted dosfstools mtools && \
      rm -rf /var/lib/apt/lists/*
WORKDIR /work
COPY sprout.efi /work/${EFI_NAME}.EFI
COPY sprout.toml /work/SPROUT.TOML
COPY kernel.efi /work/VMLINUZ
COPY shell.efi /work/SHELL.EFI
COPY xen.efi /work/XEN.EFI
COPY xen.cfg /work/XEN.CFG
COPY initramfs /work/INITRAMFS
COPY bls.conf /work/BLS.CONF
RUN truncate -s128MiB sprout.img && \
    parted --script sprout.img mklabel gpt > /dev/null 2>&1 && \
    parted --script sprout.img mkpart primary fat32 1MiB 100% > /dev/null 2>&1 && \
    parted --script sprout.img set 1 esp on > /dev/null 2>&1 && \
    mkfs.vfat -F32 -n EFI sprout.img && \
    mmd -i sprout.img ::/EFI && \
    mmd -i sprout.img ::/EFI/BOOT && \
    mmd -i sprout.img ::/LOADER && \
    mmd -i sprout.img ::/LOADER/ENTRIES && \
    mcopy -i sprout.img ${EFI_NAME}.EFI ::/EFI/BOOT/ && \
    mcopy -i sprout.img VMLINUZ ::/VMLINUZ && \
    mcopy -i sprout.img SHELL.EFI ::/EFI/BOOT/ && \
    mcopy -i sprout.img XEN.EFI ::/EFI/BOOT/ && \
    mcopy -i sprout.img XEN.CFG ::/EFI/BOOT/ && \
    mcopy -i sprout.img SPROUT.TOML ::/ && \
    mcopy -i sprout.img INITRAMFS ::/ && \
    mcopy -i sprout.img BLS.CONF ::/LOADER/ENTRIES/ && \
    mv sprout.img /sprout.img

FROM scratch AS final
COPY --from=build /sprout.img /sprout.img
