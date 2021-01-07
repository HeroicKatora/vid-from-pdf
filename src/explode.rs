/// Turn a pdf into multiple images of that each page.
use std::{collections::BTreeMap, fmt, fs, io, process::Command};
use image::{io::Reader as ImageReader, imageops};
use mupdf::Document;
use which::CanonicalPath;

use crate::FatalError;
use crate::sink::{Sink, Source};
use crate::resources::{RequiredToolError, require_tool};

pub trait ExplodePdf: Send + Sync + 'static {
    /// Create all pages as files, import them into sink.
    fn explode(&self, src: &mut dyn Source, into: &mut Sink) -> Result<(), FatalError>;
    /// Describe the pdf exploder to a `-verbose` cli user.
    fn verbose_describe(&self, into: &mut dyn io::Write) -> Result<(), FatalError>;
}

struct PdfToPpm {
    exe: CanonicalPath,
}

struct MuPdf {}

pub enum LoadPdfExploderError {
    CantFindPdfToPpm(RequiredToolError),
}

impl ExplodePdf for PdfToPpm {
    fn explode(&self, src: &mut dyn Source, sink: &mut Sink) -> Result<(), FatalError> {
        PdfToPpm::explode(self, src, sink)?;
        let paths = sink.imported().collect::<Vec<_>>();
        for mut path in paths {
            let image = ImageReader::open(&path)?
                .with_guessed_format()?
                .decode()?;
            let image = image.resize(1920, 1080, imageops::FilterType::Lanczos3);
            path.set_extension("ppm");
            image.save(&path)?;
            sink.import(path);
        }
        Ok(())
    }

    fn verbose_describe(&self, into: &mut dyn io::Write) -> Result<(), FatalError> {
        writeln!(into, "Using pdftoppm to deconstruct pdf")?;
        writeln!(into, " pdftoppm: {}", self.exe.display())?;
        Ok(())
    }
}

impl PdfToPpm {
    fn new() -> Result<PdfToPpm, LoadPdfExploderError> {
        let pdf_to_ppm = require_tool("pdftoppm")
            .map_err(LoadPdfExploderError::CantFindPdfToPpm)?;
        // TODO: version validation?
        Ok(PdfToPpm {
            exe: pdf_to_ppm,
        })
    }

    fn explode(&self, src: &mut dyn Source, sink: &mut Sink) -> Result<(), FatalError> {
        let path = match src.as_path() {
            Some(path) => path.to_owned(),
            None => sink.store_to_file(src.as_buf_read())?,
        };

        // TODO: we could fancily check that the paths do not collide.

        Command::new(&self.exe)
            .current_dir(sink.work_dir())
            .args(&["-forcenum", "-rx", "600", "-ry", "600"])
            .arg(path)
            .arg("pages")
            .status()
            .expect("Converting pdf with `pdftoppm` failed");

        let mut entries = BTreeMap::new();
        for entry in fs::read_dir(sink.work_dir())? {
            let name = entry?.file_name();
            let name = match name.to_str() {
                None => continue,
                Some(name) => name,
            };

            let file = match name.strip_suffix(".ppm") {
                Some(file) => file,
                None => continue,
            };

            let num = match file.strip_prefix("pages-") {
                Some(num) => num,
                None => continue,
            };

            let num = match num.parse::<u32>() {
                Err(_) => continue,
                Ok(num) => num,
            };

            entries.insert(num, sink.work_dir().join(name));
        }

        for (_, page) in entries.range(..) {
            sink.import(page.clone());
        }

        Ok(())
    }
}

impl dyn ExplodePdf {
    pub fn new() -> Result<Box<Self>, LoadPdfExploderError> {
        // TODO: detect if ffmpeg was compiled with librsvg.
        Ok(Box::new(MuPdf {}))
    }
}

impl fmt::Display for LoadPdfExploderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadPdfExploderError::CantFindPdfToPpm(err) => {
                write!(f, "{}", err)
            }
        }
    }
}

impl MuPdf {
    /// Rescale page and normalize placement without distorting.
    fn normalize_page_matrix(&self, bounds: mupdf::Rect) -> mupdf::Matrix {
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

    fn convert_document(&self, path: &str, sink: &mut Sink) -> Result<(), mupdf::Error> {
        let document = Document::open(path)?;

        for page in &document {
            let page = page?;
            let matrix = self.normalize_page_matrix(page.bounds()?);
            let mut svg = io::Cursor::new(page.to_svg(&matrix)?);
            let filepath = sink.store_to_file(&mut svg)?;
            sink.import(filepath);
        }

        Ok(())
    }
}

impl ExplodePdf for MuPdf {
    fn explode(&self, src: &mut dyn Source, sink: &mut Sink) -> Result<(), FatalError> {
        let path = sink.store_to_file(src.as_buf_read())?;
        match path.to_str() {
            None => Err(FatalError::Io(io::Error::new(
                io::ErrorKind::Other,
                "Non-UTF8 path is not supported",
            ))),
            Some(path) => self.convert_document(path, sink).map_err(fatal_pdf_page)
        }
    }

    fn verbose_describe(&self, into: &mut dyn io::Write) -> Result<(), FatalError> {
        writeln!(into, "Using `mupdf` to deconstruct pdf")?;
        Ok(())
    }
}

fn fatal_pdf_page(err: mupdf::Error) -> FatalError {
    FatalError::Io(io::Error::new(
        io::ErrorKind::Other,
        err
    ))
}
