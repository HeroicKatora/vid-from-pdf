# svg-to-image 

Render a single SVG to a pixmap.

The only reliable method is to shell out to `magick convert`.

With `resvg` the font will not show. Additionally, CPU based rendering is slow
and this method is even slower than the magick one. But keep an eye on this, we
might come back to it and try out `lyon` as path tessellation.

With `pathfinder` nothing works properly. Can't even get it to initialize its
GPU reliably (maybe I'm too dumb to work just based off API with little to no
doc?). And then that still does not include actually rendering a scene to the
framebuffer, and downloading the framebuffer, and wrapping it. The dependencies
are also pretty out of date.
