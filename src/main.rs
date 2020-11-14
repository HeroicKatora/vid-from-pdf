mod explode;
mod ffmpeg;
mod resources;

fn main() {
    let config = resources::Configuration::from_env();
    let resources = resources::Resources::force(&config);
    let _ = resources;
}
