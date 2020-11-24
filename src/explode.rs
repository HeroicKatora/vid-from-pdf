/// Turn a pdf into multiple images of that each page.
use std::{collections::BTreeMap, fmt, fs, io, process::Command};
use image::{io::Reader as ImageReader, imageops};
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

struct PdfExtractRs {}

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
        #[cfg(feature = "no-pdftoppm")] {
            Ok(Box::new(PdfExtractRs {}))
        }
        #[cfg(not(eature = "no-pdftoppm"))] {
            Ok(Box::new(PdfToPpm::new()?))
        }
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

#[cfg(feature = "no-pdftoppm")]
impl ExplodePdf for PdfExtractRs {
    fn explode(&self, src: &mut dyn Source, sink: &mut Sink) -> Result<(), FatalError> {
        use pdf_extract::{MediaBox, OutputError, Transform};

        struct PagedSvg<'sink> {
            sink: &'sink mut Sink,
            file: Option<io::BufWriter<fs::File>>,
        }

        impl PagedSvg<'_> {
            fn in_page(&mut self) -> SVGOutput<'_> {
                SVGOutput::new(self.file.as_mut().unwrap())
            }
        }

        impl pdf_extract::OutputDev for PagedSvg<'_> {
            fn begin_page(
                &mut self,
                page_num: u32,
                media_box: &MediaBox,
                art_box: Option<(f64, f64, f64, f64)>,
            ) -> Result<(), OutputError> {
                let unique = self.sink.unique_path().unwrap();
                let file = fs::OpenOptions::new().create(true).write(true).open(unique.path)?;
                self.file = Some(io::BufWriter::new(file));
                self.in_page().begin_page(page_num, media_box, art_box)
            }
            fn end_page(&mut self) -> Result<(), OutputError> {
                use std::io::Write as _;
                self.in_page().end_page()?;
                let mut file = self.file.take().unwrap();
                file.flush()?;
                Ok(())
            }
            fn output_character(
                &mut self,
                trm: &Transform,
                width: f64,
                spacing: f64,
                font_size: f64,
                char: &str
            ) -> Result<(), OutputError> {
                self.in_page().output_character(trm, width, spacing, font_size, char)
            }
            fn begin_word(&mut self) -> Result<(), OutputError> {
                self.in_page().begin_word()
            }
            fn end_word(&mut self) -> Result<(), OutputError> {
                self.in_page().end_word()
            }
            fn end_line(&mut self) -> Result<(), OutputError> {
                self.in_page().end_line()
            }
        }

        use lopdf::Document;
        use pdf_extract::{SVGOutput, output_doc};

        let document = if let Some(path) = src.as_path() {
            // FIXME: error
            Document::load(path).unwrap()
        } else {
            let file = sink.store_to_file(src.as_buf_read())?;
            // FIXME: error
            Document::load(file).unwrap()
        };

        let mut paged = PagedSvg { sink, file: None };
        // FIXME: error
        output_doc(&document, &mut paged).unwrap();

        todo!()
    }

    fn verbose_describe(&self, into: &mut dyn io::Write) -> Result<(), FatalError> {
        writeln!(into, "Using Rust crate `pdf-extract` to deconstruct pdf")?;
        Ok(())
    }
}
