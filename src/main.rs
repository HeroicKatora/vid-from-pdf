mod explode;
mod ffmpeg;
mod resources;
mod sink;

use std::fmt;

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
    }
    Ok(())
}

pub enum FatalError {
    Io(std::io::Error),
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
            FatalError::Io(io) => write!(f, "{:?}", io),
        }
    }
}
