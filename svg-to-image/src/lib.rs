//! A glue crate for rendering an svg to a pixmap that can be saved.
use std::{io, path::Path};

pub struct Svg {
    pub tree: usvg::Tree,
    magick: MagickConvert,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Clone)]
pub struct MagickConvert {
    magick: which::CanonicalPath,
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
    RequiredTool {
        tool: &'static str,
        information: Option<Box<str>>,
    },
    // No further information.
    Resvg,
    UnsupportedRenderMethod(&'static str),
}

impl Svg {
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
            self.render_convert(&self.magick)
        }
    }

    fn render_convert(&self, magick: &MagickConvert) -> Result<image::DynamicImage, Error> {
        let tree = self.tree.to_string(Default::default());
        let exec = subprocess::Exec::cmd(&magick.magick)
            .arg("convert")
            .arg("svg:-")
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

impl MagickConvert {
    pub const MAGICK: &'static str = "magick";

    pub fn new(magick: which::CanonicalPath) -> Result<Self, Error> {
        let formats = subprocess::Exec::cmd(&magick)
            .arg("identify")
            .arg("-list")
            .arg("format")
            .stdin(subprocess::Redirection::None)
            .stdout(subprocess::Redirection::Pipe)
            .stderr(subprocess::Redirection::Pipe)
            // Should we limit the output?
            .capture()?;

        let lines = match String::from_utf8(formats.stdout) {
            Err(_) => return Err(Error {
                kind: ErrorKind::RequiredTool {
                    tool: "convert",
                    information: None,
                },
            }),
            Ok(string) => string,
        };

        if let Some(true) = Self::check_svg_read(&lines) {} else {
            return Err(Error {
                kind: ErrorKind::RequiredTool {
                    tool: "convert",
                    information: Some("SVG read support".into()),
                }
            });
        }

        if let Some(true) = Self::check_ppm_write(&lines) {} else {
            return Err(Error {
                kind: ErrorKind::RequiredTool {
                    tool: "convert",
                    information: Some("PPM write support".into()),
                }
            });
        }

        Ok(MagickConvert {
            magick,
        })
    }

    pub fn path(&self) -> &Path {
        self.magick.as_path()
    }

    pub fn open(&self, path: &Path) -> Result<Svg, Error> {
        let mut options = usvg::Options::default();
        options.fontdb.load_system_fonts();

        if options.fontdb.is_empty() {
            panic!("failed to find system fonts for loading");
        }

        let tree = usvg::Tree::from_file(path, &options)?;
        Ok(Svg {
            tree,
            magick: self.clone(),
        })
    }

    /// Prepare converting a particular SVG tree.
    pub fn with_tree(&self, tree: usvg::Tree) -> Svg {
        Svg {
            tree,
            magick: self.clone(),
        }
    }

    fn check_svg_read(st: &str) -> Option<bool> {
        Self::check_format_support(st, "SVG", |mode| {
            Some('r') == mode.chars().next()
        })
    }

    fn check_ppm_write(st: &str) -> Option<bool> {
        Self::check_format_support(st, "PPM", |mode| {
            Some('w') == mode.chars().nth(1)
        })
    }

    fn check_format_support(st: &str, format: &str, test_mode: impl Fn(&str) -> bool)
        -> Option<bool>
    {
        let svg_spec = st
            .lines()
            .filter(|line| line.contains(format));

        for line in svg_spec {
            // Each line in the format table is of the form:
            //    Format  Module    Mode  Description
            // Mode is a subset of rw+, where + means multiple images per file.
            let line = line.trim_start();

            if !line.starts_with(format) {
                continue;
            }

            let format = if let Some(pos) = line.find(' ') {
                line[pos..].trim_start()
            } else {
                continue;
            };

            let mode = if let Some(pos) = format.find(' ') {
                format[pos..].trim_start()
            } else {
                continue;
            };

            if test_mode(mode) {
                return Some(true);
            }
        }

        Some(false)
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
    let magic = which::CanonicalPath::new("magick")
        .expect("Magick convert not found");
    let convert = MagickConvert::new(magic)
        .expect("Magick does not support required format.");

    let svg = convert.open(Path::new(path))
        .expect("Failed to read example svg");
    let image = svg.render()
        .expect("Failed to render");
    assert_eq!(image.width(), 1920);
    assert_eq!(image.height(), 1440);
}
