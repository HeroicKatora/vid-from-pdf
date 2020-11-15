/// Turn a pdf into multiple images of that each page.
use std::{fmt, fs, io, path::Path, path::PathBuf, process::Command};
use which::CanonicalPath;

use crate::FatalError;
use crate::sink::Sink;
use crate::resources::{RequiredToolError, require_tool};

pub trait ExplodePdf: Send + Sync + 'static {
    /// Create all pages as files, import them into sink.
    fn explode(&self, src: &mut dyn Source, into: &mut Sink) -> Result<(), FatalError>;
    /// Describe the pdf exploder to a `-verbose` cli user.
    fn verbose_describe(&self, into: &mut dyn io::Write) -> Result<(), FatalError>;
}

pub trait Source {
    fn as_buf_read(&mut self) -> &mut dyn io::BufRead;
    fn as_path(&self) -> Option<&Path>;
}

pub struct FileSource {
    file: io::BufReader<fs::File>,
    path: PathBuf,
}

struct PdfToPpm {
    exe: CanonicalPath,
}

pub enum LoadPdfExploderError {
    CantFindPdfToPpm(RequiredToolError),
}

impl ExplodePdf for PdfToPpm {
    fn explode(&self, src: &mut dyn Source, sink: &mut Sink) -> Result<(), FatalError> {
        PdfToPpm::explode(self, src, sink)
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
            .args(&["-png", "-rx", "600", "-ry", "600"])
            .arg(path)
            .arg("pages")
            .status()
            .expect("Converting pdf with `pdftoppm` failed");

        for idx in 0.. {
            let frame = format!("pages-{}.png", idx + 1);
            let frame = sink.work_dir().join(&frame);
            if !frame.exists() {
                break;
            }
            sink.import(frame);
        }

        Ok(())
    }
}

impl FileSource {
    /// Create by opening a file assumed to exist.
    pub fn new_from_existing(path: PathBuf) -> Result<Self, FatalError> {
        let file = fs::File::open(&path)?;
        Ok(FileSource {
            file: io::BufReader::new(file),
            path,
        })
    }
}

impl Source for FileSource {
    fn as_buf_read(&mut self) -> &mut dyn io::BufRead {
        &mut self.file
    }

    fn as_path(&self) -> Option<&Path> {
        Some(&self.path)
    }
}

impl dyn ExplodePdf {
    pub fn new() -> Result<Box<Self>, LoadPdfExploderError> {
        let pdf_to_ppm = PdfToPpm::new()?;
        Ok(Box::new(pdf_to_ppm))
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
