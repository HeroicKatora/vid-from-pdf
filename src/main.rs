mod explode;
mod ffmpeg;
mod resources;
mod sink;

fn main() -> Result<(), FatalError> {
    let config = resources::Configuration::from_env();
    let resources = resources::Resources::force(&config);
    let _ = resources;
    Ok(())
}

#[derive(Debug)]
pub enum FatalError {
    Io(std::io::Error),
}

impl From<std::io::Error> for FatalError {
    fn from(err: std::io::Error) -> FatalError {
        FatalError::Io(err)
    }
}
