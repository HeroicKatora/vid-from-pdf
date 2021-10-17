mod encoder;
mod missing_specs;
mod paged_vec;

use std::{collections::HashMap, io, io::Write, fs, path::PathBuf, process};
use serde::{Deserialize, Serialize};
use self::encoder::{Encoder, Error, SlideShow};
use self::paged_vec::PagedVec;

fn main() {
    if let Err(_) = main_with_stdio() {
        process::exit(2);
    }
}

fn main_with_stdio() -> Result<(), io::Error> {
    let config = match serde_json::from_reader::<_, Config>(io::stdin()) {
        Err(err) => {
            let err = format!("{:?}", err);
            serde_json::to_writer(io::stdout(), &CallResult::Err(err))?;
            process::exit(1);
        }
        Ok(config) => config,
    };

    match assemble_file(config)? {
        Err(err) => {
            let err = format!("{:?}", err);
            serde_json::to_writer(io::stdout(), &CallResult::Err(err))?;
            process::exit(1);
        }
        Ok(()) => {
            serde_json::to_writer(io::stdout(), &CallResult::Ok)?;
            process::exit(0);
        }
    }
}

fn assemble_file(config: Config)
    -> Result<Result<(), Error>, io::Error>
{
    let slide_show = match config.slides.first() {
        None => return Ok(Err(Error)),
        // Use the first slide to derive the metadata.
        Some(first_slide) => {
            let show = match basic_show_data(first_slide)? {
                Err(err) => return Ok(Err(err)),
                Ok(show) => show,
            };

            SlideShow {
                slides: &config.slides,
                ..show
            }
        }
    };

    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&config.target)?;

    let page = PagedVec::new(config.memory);
    let mut encoder = Encoder::new(&slide_show, page);

    while !encoder.done() {
        match encoder.step()? {
            Ok(()) => {},
            Err(err) => return Ok(Err(err))
        }

        let page = encoder.ready();
        file.write_all(page)?;
        let consumed = page.len();
        encoder.consume(consumed);
    }

    file.write_all(encoder.tail()).map(Ok)
}

fn basic_show_data(slide: &Slide)
    -> Result<Result<SlideShow<'static>, Error>, io::Error>
{
    let reader = image::io::Reader::open(&slide.image)?;

    let (width, height) = match reader.into_dimensions() {
        Ok(dims) => dims,
        Err(image::ImageError::IoError(io)) => return Err(io),
        Err(_other) => return Ok(Err(Error)),
    };

    Ok(Ok(SlideShow {
        slides: &[],
        width,
        height,
        color: encoder::Color::Srgb,
        audio: encoder::Audio::Raw16BitLE,
    }))
}

#[derive(Serialize)]
enum CallResult {
    #[serde(rename = "error")] 
    Err(String),
    #[serde(rename = "ok")] 
    Ok,
}

#[derive(Deserialize)]
struct Config {
    target: PathBuf,
    slides: Vec<Slide>,
    #[serde(default = "PagedVec::default_memory")]
    memory: usize,
}

#[derive(Deserialize)]
pub struct Slide {
    /// The rendered frame images according to `Color` in slide show.
    pub image: PathBuf,
    /// The rendered audio track according to `Audio` in slide show.
    pub audio: PathBuf,
    /// Textual representation of this frame.
    /// We're not responsible for breaking it apart in intervals that make sense and leave enough
    /// screen space to read. Maps language to the list of texts.
    pub subtitles: HashMap<String, Subtitle>,
    /// A title name to give to this frame.
    pub chapter: Option<Chapter>,
}

#[derive(Deserialize)]
pub struct Subtitle {
    pub text: String,
    pub timing: f32,
}

#[derive(Deserialize)]
pub struct Chapter {
    pub title: String,
    pub depth: usize,
}
