//! A glue crate for rendering an svg to a pixmap that can be saved.
use std::{io, path::Path};

pub struct Svg {
    pub tree: usvg::Tree,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    Io(io::Error),
    Usvg(usvg::Error),
    Image(image::ImageError),
    Popen(subprocess::PopenError),
    Convert {
        status: subprocess::ExitStatus,
        stderr: Vec<u8>,
    },
    // No further information.
    Resvg,
    UnsupportedRenderMethod(&'static str),
}

impl Svg {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let mut options = usvg::Options::default();
        options.fontdb.load_system_fonts();

        if options.fontdb.is_empty() {
            panic!("failed to find system fonts for loading");
        }

        let tree = usvg::Tree::from_file(path, &options)?;
        Ok(tree.into())
    }

    pub fn render(&self) -> Result<image::DynamicImage, Error> {
        // choose renderer.
        if cfg!(render_pathfinder) {
            let size = self.tree.svg_node().size.to_screen_size();
            let width = size.width();
            let height = size.height();

            let mut image = image::RgbaImage::new(width, height);
            self.render_pathfinder_gl(&mut image)?;

            return Ok(image::DynamicImage::ImageRgba8(image));
        } else if cfg!(render_resvg) {
            let size = self.tree.svg_node().size.to_screen_size();
            let width = size.width();
            let height = size.height();

            let mut image = image::RgbaImage::new(width, height);
            self.render_resvg(&mut image)?;

            return Ok(image::DynamicImage::ImageRgba8(image));
        } else {
            self.render_convert()
        }
    }

    fn render_convert(&self) -> Result<image::DynamicImage, Error> {
        let tree = self.tree.to_string(Default::default());
        let exec = subprocess::Exec::cmd("convert")
            .arg("-")
            .arg("ppm:-")
            .stdin(tree.into_bytes())
            .stdout(subprocess::Redirection::Pipe)
            .stderr(subprocess::Redirection::Pipe)
            .capture()?;

        if !exec.success() {
            return Err(Error {
                kind: ErrorKind::Convert {
                    status: exec.exit_status,
                    stderr: exec.stderr,
                },
            });
        }

        let image_data = io::Cursor::new(exec.stdout);
        let image = image::io::Reader::with_format(image_data, image::ImageFormat::Pnm)
            .decode()?;
        Ok(image)
    }

    #[cfg(not(pathfinder))]
    fn render_pathfinder_gl(&self, _: &mut image::RgbaImage) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedRenderMethod("pathfinder"),
        })
    }

    #[cfg(pathfinder)]
    fn render_pathfinder_gl(&self, image: &mut image::RgbaImage) -> Result<(), Error> {
        let width = image.width();
        let height = image.height();

        gl::load_with(|s| glfw::Glfw.get_proc_address_raw(s));

        let svg = {
            let string = self.tree.to_string(Default::default());
            let options = old_usvg::Options::default();
            let tree = old_usvg::Tree::from_str(&string, &options).unwrap();
            pathfinder_svg::BuiltSVG::from_tree(&tree);
        };

        let resources = pathfinder_resources::embedded::EmbeddedResourceLoader::new();

        let gl = pathfinder_gl::GLDevice::new(
            pathfinder_gl::GLVersion::GLES3,
            0);

        use pathfinder_gpu::Device;
        let texture = gl.create_texture(
            pathfinder_gpu::TextureFormat::RGBA8,
            pathfinder_geometry::vector::Vector2I::new(width as i32, height as i32),
        );

        let framebuffer = gl.create_framebuffer(texture);

        let renderer = pathfinder_renderer::gpu::renderer::Renderer::new(
            gl,
            &resources,
            pathfinder_renderer::gpu::options::DestFramebuffer::Other(framebuffer),
            Default::default());

        Ok(())
    }

    #[cfg(not(render_resvg))]
    fn render_resvg(&self, image: &mut image::RgbaImage) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedRenderMethod("resvg"),
        })
    }

    // FIXME(2021-Jan): this fails to render the text.
    #[cfg(render_resvg)]
    fn render_resvg(&self, image: &mut image::RgbaImage) -> Result<(), Error> {
        let width = image.width();
        let height = image.height();

        let pixmap = tiny_skia::PixmapMut::from_bytes(image, width, height)
            .expect("Correct size for buffer");

        match resvg::render(&self.tree, usvg::FitTo::Original, pixmap) {
            None => Err(Error::failed_to_render()),
            Some(()) => Ok(()),
        }
    }
}

impl From<usvg::Tree> for Svg {
    fn from(tree: usvg::Tree) -> Self {
        Svg { tree }
    }
}

impl Error {
    fn failed_to_render() -> Self {
        Error {
            kind: ErrorKind::Resvg,
        }
    }
}

impl From<usvg::Error> for Error {
    fn from(err: usvg::Error) -> Self {
        Error {
            kind: ErrorKind::Usvg(err),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error {
            kind: ErrorKind::Io(err),
        }
    }
}

impl From<image::ImageError> for Error {
    fn from(err: image::ImageError) -> Self {
        Error {
            kind: ErrorKind::Image(err),
        }
    }
}

impl From<subprocess::PopenError> for Error {
    fn from(err: subprocess::PopenError) -> Self {
        Error {
            kind: ErrorKind::Popen(err),
        }
    }
}

#[test]
fn simple() {
    use image::GenericImageView;

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/test.svg");
    let svg = Svg::open(Path::new(path))
        .expect("Failed to read example svg");
    let image = svg.render()
        .expect("Failed to render");
    assert_eq!(image.width(), 1920);
    assert_eq!(image.height(), 1440);
}
