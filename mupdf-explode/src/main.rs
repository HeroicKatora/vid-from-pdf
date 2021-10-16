use std::{fs, io, path::PathBuf, process};
use mupdf::{Document, Error};
use serde::{Deserialize, Serialize};

fn main() {
    if let Err(_) = main_with_stdio() {
        process::exit(2);
    }
}

fn main_with_stdio() -> Result<(), io::Error> {
    let config = match serde_json::from_reader::<_, Config>(io::stdin()) {
        Err(err) => {
            let err = format!("{:?}", err);
            serde_json::to_writer(io::stdout(), &CallResult::Err(err))?;
            process::exit(1);
        }
        Ok(config) => config,
    };

    match convert_document(config) {
        Err(err) => {
            let err = format!("{:?}", err);
            serde_json::to_writer(io::stdout(), &CallResult::Err(err))?;
            process::exit(1);
        }
        Ok(paths) => {
            serde_json::to_writer(io::stdout(), &CallResult::Ok(paths))?;
            process::exit(0);
        }
    }
}

#[derive(Serialize)]
enum CallResult {
    #[serde(rename = "error")] 
    Err(String),
    #[serde(rename = "ok")] 
    Ok(Vec<PathBuf>),
}

#[derive(Deserialize)]
struct Config {
    target_dir: PathBuf,
    path: String,
}

struct Conversion {
    cfg: Config,
    page: usize,
}

fn convert_document(cfg: Config)
    -> Result<Vec<PathBuf>, Error>
{
    let document = Document::open(&cfg.path)?;
    let mut conversion = Conversion {
        cfg,
        page: 0,
    };

    let mut paths = vec![];
    for page in &document {
        let page = page?;
        let matrix = normalize_page_matrix(page.bounds()?);
        let mut svg = io::Cursor::new(page.to_svg(&matrix)?);
        let filepath = store_to_file(&mut conversion, &mut svg)?;
        paths.push(filepath);
        conversion.page += 1;
    }

    Ok(paths)
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

fn store_to_file(conv: &mut Conversion, content: &mut dyn io::BufRead)
    -> Result<PathBuf, Error>
{
    let filename = format!("page-{}.svg", conv.page);
    let filepath = conv.cfg.target_dir.join(filename);

    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&filepath)?;

    io::copy(content, &mut file)?;
    Ok(filepath)
}
