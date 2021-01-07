
# Building

The build is mostly orchestrated by cargo and will detect several dependencies
at runtime, if possible. For static dependencies see below.

## Requirements

Runtime:
* `ffmpeg`, the `avcodec` library. It will detect support for the `h264`
  encoder using either `nvenc`, `vdpau`, or the software encoder (very slow) in
  that order of priority.
* `ffprobe`
* `pdftoppm` when not built with `mupdf`.

## With pdftoppm

Requires the poppler utils. Do note that the initial detection of pages is kind
of slow, and will produce large images. (This is because the tool lacks the
possibility to scale-to-fit to width and height while maintaining aspect
ratio). The intermediate pixmaps may be multiple megabytes in size and
rasterization is not very fast.

## With mupdf

This pdf reader will render pages to SVG, then rasterize them using `resvg`.
This is generally much faster in the initial step than `pdftoppm`. When
building with `musl` then the tools `musl-gcc` and `xxd` are required.
