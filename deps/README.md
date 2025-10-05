# Vendored Dependencies

Currently, sprout requires some vendored dependencies to work around usage of simd.

Both `moxcms` and `simd-adler32` are used for the image library for the splash screen feature.

## moxcms

- Removed NEON, SSE, and AVX support.

## simd-adler2

- Made compilation function on UEFI targets.
