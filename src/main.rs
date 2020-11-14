mod app;
mod cli;
mod explode;
mod ffmpeg;
mod project;
mod resources;
mod sink;

use std::fmt;

static COMPRESSED_DEPENDENCY_LIST: &[u8] = auditable::inject_dependency_list!();

fn main() -> Result<(), FatalError> {
    let mut cfg = resources::Configuration::from_env()?;
    let resources = resources::Resources::force(&cfg)?;
    if cfg.verbose {
        use std::io::Write as _;
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
    cli::tui(app)?;
    Ok(())
}

pub enum FatalError {
    Io(std::io::Error),
    /// A corrupt, __internal__ data dump.
    Corrupt(serde_json::Error),
}

impl From<std::io::Error> for FatalError {
    fn from(err: std::io::Error) -> FatalError {
        FatalError::Io(err)
    }
}

impl fmt::Debug for FatalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "The program will quit due to a fatal error.")?;
        writeln!(f, "This should never happen and might be caused by a bad installation.")?;
        match self {
            FatalError::Io(io) => write!(f, "I/O error: {:?}", io),
            FatalError::Corrupt(err) => write!(f, "Corrupt data structure: {:?}", err),
        }
    }
}
