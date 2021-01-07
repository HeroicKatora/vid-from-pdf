//! A glue crate for rendering an svg to a pixmap that can be saved.
use std::{io, path::Path};

pub struct Svg {
    pub tree: usvg::Tree,
}

#[derive(Debug)]
pub struct Error {
    _private: u8,
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
        let size = self.tree.svg_node().size.to_screen_size();
        let width = size.width();
        let height = size.height();

        let mut image = image::RgbaImage::new(width, height);
        let pixmap = tiny_skia::PixmapMut::from_bytes(&mut image, width, height)
            .expect("Correct size for buffer");

        match resvg::render(&self.tree, usvg::FitTo::Original, pixmap) {
            None => return Err(Error::failed_to_render()),
            Some(()) => {},
        }

        Ok(image::DynamicImage::ImageRgba8(image))
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
            _private: 0,
        }
    }
}

impl From<usvg::Error> for Error {
    fn from(err: usvg::Error) -> Self {
        Error {
            _private: 0,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error {
            _private: 0,
        }
    }
}

impl From<image::ImageError> for Error {
    fn from(err: image::ImageError) -> Self {
        Error {
            _private: 0,
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
    let _ = image.save("debug.png");
    assert_eq!(image.width(), 1920);
    assert_eq!(image.height(), 1440);
}
