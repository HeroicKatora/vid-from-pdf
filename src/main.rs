mod explode;
mod ffmpeg;
mod resources;
mod sink;

use std::fmt;

fn main() -> Result<(), FatalError> {
    let config = resources::Configuration::from_env()?;
    let resources = resources::Resources::force(&config);
    let _ = resources;
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
