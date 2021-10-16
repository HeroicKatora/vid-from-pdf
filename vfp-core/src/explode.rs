/// Turn a pdf into multiple images of that each page.
use std::{io, path::PathBuf};

use serde_json::{json, Value};
use subprocess::{Exec, Redirection};

use crate::FatalError;
use crate::sink::{Sink, Source};

pub trait ExplodePdf: Send + Sync + 'static {
    /// Create all pages as files, import them into sink.
    fn explode(&self, src: &mut dyn Source, into: &mut Sink) -> Result<(), FatalError>;
    /// Describe the pdf exploder to a `-verbose` cli user.
    fn verbose_describe(&self, into: &mut dyn io::Write) -> Result<(), FatalError>;
}

struct MuPdf {
    binary: PathBuf,
}

impl dyn ExplodePdf {
    pub fn new(sink: &mut Sink) -> Result<Box<Self>, FatalError> {
        // TODO: detect if ffmpeg was compiled with librsvg.
        let mupdf = MuPdf::new(sink)?;
        Ok(Box::new(mupdf))
    }
}

impl MuPdf {
    const FILE: &'static [u8] = include_bytes!(env!("VFP_MUPDF_EXPLODE"));

    fn new(sink: &mut Sink) -> Result<Self, FatalError> {
        let binary = sink.store_to_file(&mut {Self::FILE})?;
        
        if cfg!(unix) {
            use std::fs;
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o700);
            fs::set_permissions(&binary, perms)?;
        }

        Ok(MuPdf {
            binary,
        })
    }

    fn convert_document(&self, path: &str, sink: &mut Sink) -> Result<(), FatalError> {
        let tempdir = sink.unique_mkdir()?;
        let input = json!({
                "path": path,
                "target_dir": &tempdir.path,
            });
        let input = serde_json::to_vec(&input)?;

        let capture = Exec::cmd(&self.binary)
            .stdin(input)
            .stdout(Redirection::Pipe)
            .capture()
            .map_err(|_| {
                FatalError::ConversionFailed
            })?;

        let output: Value = serde_json::from_slice(&capture.stdout)
            .map_err(|err| {
                eprintln!("Stdout: {:?}", capture.stdout_str());
                eprintln!("Error: {:?}", err);
                err
            })?;

        if let Some(_) = output.get("error") {
            return Err(FatalError::ConversionFailed);
        }

        let outputs = output.get("ok")
            .ok_or(FatalError::ConversionFailed)?
            .as_array()
            .ok_or(FatalError::ConversionFailed)?;

        for item in outputs {
            let path = item.as_str().ok_or(FatalError::ConversionFailed)?;
            sink.import(PathBuf::from(path));
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
            Some(path) => self.convert_document(path, sink),
        }
    }

    fn verbose_describe(&self, into: &mut dyn io::Write) -> Result<(), FatalError> {
        writeln!(into, "Using `mupdf` to deconstruct pdf")?;
        Ok(())
    }
}
