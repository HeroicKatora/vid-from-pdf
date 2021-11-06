mod encoder;
mod missing_specs;
mod paged_vec;

use std::{collections::HashMap, io, io::Write, fs, path::PathBuf, process};
use image::{io::Reader as ImageReader, ImageError};
use serde::{Deserialize, Serialize};

use self::encoder::{Encoder, Error, ErrorKind, SlideShow};
use self::paged_vec::PagedVec;

fn main() {
    if let Err(err) = main_with_stdio() {
        eprintln!("{:?}", err);
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
        Ok(file) => {
            serde_json::to_writer(io::stdout(), &CallResult::Ok(file))?;
            process::exit(0);
        }
    }
}

fn assemble_file(config: Config)
    -> Result<Result<FileResult, Error>, io::Error>
{
    let slide_show = match config.slides.first() {
        None => return Ok(Err(ErrorKind::EmptySequence.into())),
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

    let mut length = 0;
    while !encoder.done() {
        match encoder.step()? {
            Ok(()) => {},
            Err(err) => return Ok(Err(err))
        }

        let page = encoder.ready();
        file.write_all(&*page)?;
        let consumed = page.len();
        drop(page);

        length += consumed;
        encoder.consume(consumed);
    }

    let tail = encoder.tail();
    file.write_all(&*tail)?;
    length += tail.len();
    drop(tail);

    file.flush()?;

    Ok(Ok(FileResult {
        length: length as u64,
    }))
}

fn basic_show_data(slide: &Slide)
    -> Result<Result<SlideShow<'static>, Error>, io::Error>
{
    let reader = ImageReader::open(&slide.image)?;

    let (width, height) = match reader.into_dimensions() {
        Ok(dims) => dims,
        Err(ImageError::IoError(io)) => return Err(io),
        Err(err) => return Ok(Err(err.into())),
    };

    let mut audio_file;
    // FIXME: really we only need the header here but oh well.
    let reader = wav::read({
        audio_file = fs::File::open(&slide.audio)?;
        &mut audio_file
    });

    let (header, _) = match convert_wav_result(reader) {
        Ok(items) => items,
        Err(other) => return other.map(Err),
    };

    let sps = header.bytes_per_second / u32::from(header.bytes_per_sample);

    Ok(Ok(SlideShow {
        slides: &[],
        width,
        height,
        color: encoder::Color::Srgb,
        audio: encoder::Audio::Pcm {
            sampling_frequency: sps,
            channels: header.channel_count,
            bits_per_sample: header.bits_per_sample,
        },
    }))
}

fn convert_wav_result<T>(result: Result<T, io::Error>)
    -> Result<T, Result<Error, io::Error>>
{
    match result {
        Ok(items) => Ok(items),
        // `wav` signals a decoding failure like this.
        Err(io) if io.kind() == io::ErrorKind::Other => {
            return Err(Ok(ErrorKind::Wav(io).into()));
        },
        Err(io) => return Err(Err(io)),
    }
}

#[derive(Serialize)]
enum CallResult {
    #[serde(rename = "error")] 
    Err(String),
    #[serde(rename = "ok")] 
    Ok(FileResult),
}

#[derive(Deserialize)]
struct Config {
    target: PathBuf,
    slides: Vec<Slide>,
    #[serde(default = "PagedVec::default_memory")]
    memory: usize,
}

#[derive(Serialize)]
struct FileResult {
    length: u64,
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
    /// How many seconds this slide should last.
    pub seconds: f32,
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
