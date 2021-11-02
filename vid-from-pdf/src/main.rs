mod app;
mod resources;
mod web;

use std::fmt;
use std::io::Write as _;

static COMPRESSED_DEPENDENCY_LIST: &[u8] = auditable::inject_dependency_list!();

fn main() {
    if let Err(err) = run() {
        eprintln!("{:?}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<(), FatalError> {
    let mut cfg = resources::Configuration::from_env()?;
    let resources = resources::Resources::force(&cfg)?;
    if cfg.verbose {
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
    web::serve(app)?;

    Ok(())
}


pub enum FatalError {
    Io(std::io::Error),
    /// A corrupt, __internal__ data dump.
    Corrupt(serde_json::Error),
    /// An input slide that we could not convert to a pixmap.
    /// This is a theoretical concern as everything is SVG which we try to render. However, just
    /// preparing for future ideas where this might be more dynamic.
    UnrecognizedInputSlide,
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
            FatalError::Io(io) => write!(f, "I/O error: {}", io),
            FatalError::Corrupt(err) => write!(f, "Corrupt data structure: {:?}", err),
            FatalError::UnrecognizedInputSlide => write!(f, "An input slide was in unrecognized image format after conversion"),
        }
    }
}