mod app;
mod cli;
mod explode;
mod ffmpeg;
mod project;
mod resources;
mod sink;
#[cfg(test)]
mod test;
mod web;

use std::fmt;
use std::io::Write as _;

static COMPRESSED_DEPENDENCY_LIST: &[u8] = auditable::inject_dependency_list!();

fn main() -> Result<(), FatalError> {
    let mut cfg = resources::Configuration::from_env()?;
    let resources = resources::Resources::force(&cfg)?;
    if cfg.verbose {
        writeln!(cfg.stderr, "Using ffmpeg")?;
        writeln!(cfg.stderr, " ffmpeg: {}", resources.ffmpeg.ffmpeg.as_path().display())?;
        writeln!(cfg.stderr, " ffprobe: {}", resources.ffmpeg.ffprobe.as_path().display())?;
        writeln!(cfg.stderr, " version: {}", resources.ffmpeg.version.version)?;
        writeln!(cfg.stderr, "Using temporary directory")?;
        writeln!(cfg.stderr, " path: {}", resources.tempdir.path().display())?;
        resources.explode.verbose_describe(&mut cfg.stderr)?;

        writeln!(cfg.stderr, "There is `auditable` information")?;
        if let Some(_) = std::env::var_os("VID_FROM_PDF_DUMP_AUDITABLE") {
            // Firstly, this actually uses the `COMPRESSED_DEPENDENCY_LIST` ensuring it is not
            // removed during a linker stage. Secondly, maybe it's useful.
            writeln!(cfg.stderr, " Dumping as a C-compatible escape byte array.")?;
            let mut locked = cfg.stderr.lock();
            write!(locked, "'")?;
            for &ch in COMPRESSED_DEPENDENCY_LIST {
                write!(locked, "{}", std::ascii::escape_default(ch))?;
            }
            write!(locked, "'")?;
        }
            
    }
    let app = app::App::new(resources);

    if crossterm::tty::IsTty::is_tty(&cfg.stdout) && !cfg.force_web {
        cli::tui(app)?;
        writeln!(cfg.stdout, "")?;
    } else {
        web::serve(app)?;
    }

    Ok(())
}

pub enum FatalError {
    Io(std::io::Error),
    /// A corrupt, __internal__ data dump.
    Corrupt(serde_json::Error),
    /// Some error in image conversion.
    Image(image::error::ImageError),
    /// We passed a really bad parameter to apng encoding.
    Apng(png::EncodingError),
}

impl From<std::io::Error> for FatalError {
    fn from(err: std::io::Error) -> FatalError {
        FatalError::Io(err)
    }
}

impl From<image::error::ImageError> for FatalError {
    fn from(err: image::error::ImageError) -> FatalError {
        FatalError::Image(err)
    }
}

impl From<png::EncodingError> for FatalError {
    fn from(err: png::EncodingError) -> FatalError {
        FatalError::Apng(err)
    }
}

impl fmt::Debug for FatalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "The program will quit due to a fatal error.")?;
        writeln!(f, "This should never happen and might be caused by a bad installation.")?;
        match self {
            FatalError::Io(io) => write!(f, "I/O error: {:?}", io),
            FatalError::Corrupt(err) => write!(f, "Corrupt data structure: {:?}", err),
            FatalError::Image(err) => write!(f, "Bad image data: {:?}", err),
            FatalError::Apng(err) => write!(f, "Bad apng sequence: {:?}", err),
        }
    }
}
