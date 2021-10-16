use std::{io, path::PathBuf};
use mupdf::{Document, Error};

fn main() {
}

struct Config {
    target_dir: PathBuf,
}

fn convert_document(cfg: &Config, path: &str)
    -> Result<Vec<PathBuf>, Error>
{
    let document = Document::open(path)?;

    for page in &document {
        let page = page?;
        let matrix = normalize_page_matrix(page.bounds()?);
        let mut svg = io::Cursor::new(page.to_svg(&matrix)?);
        // let filepath = sink.store_to_file(&mut svg)?;
        // sink.import(filepath);
    }

    todo!()
}

/// Rescale page and normalize placement without distorting.
fn normalize_page_matrix(bounds: mupdf::Rect) -> mupdf::Matrix {
    let (width, height) = (bounds.width(), bounds.height());
    let origin = bounds.origin();

    let mut matrix = mupdf::Matrix::IDENTITY;
    let scale_w = 1920.0/width;
    let scale_h = 1080.0/height;
    // Scale to contain.
    let scale = scale_w.min(scale_h);
    matrix.pre_translate(-origin.x, -origin.y);
    matrix.scale(scale, scale);

    matrix
}

