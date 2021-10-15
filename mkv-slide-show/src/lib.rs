use std::collections::HashMap;
mod paged_vec;

use std::path::PathBuf;
use self::paged_vec::PagedVec;

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

pub enum Color {
    Srgb,
}

pub enum Audio {
    Raw16BitLE,
}

pub struct Subtitle {
    pub text: String,
    pub timing: f32,
}

pub struct Chapter {
    pub title: String,
    pub depth: usize,
}

pub struct SlideShow<'slides> {
    pub slides: &'slides [Slide],
    pub width: u32,
    pub height: u32,
    pub color: Color,
    pub audio: Audio,
}

pub struct Encoder<'slides> {
    slides: &'slides [Slide],
    audio: AudioTrack,
    video: VideoTrack,
    progress: Progress,
}

/// Internal state keeping.
enum Progress {
    Initial,
    BeforeFrame(usize),
    Done,
}

/// The 'interface' of how we chose to encode audio.
///
/// That is, the Matroska mapped version that we computed from the input parameters—the string
/// name, the sampling frequency, the track ID etc.
struct AudioTrack {
}

/// The 'interface' of how we chose to encode video.
///
/// That is, the Matroska mapped version that we computed from the input parameters—the string
/// name of the codec, its track ID, etc.
struct VideoTrack {
}

impl<'slides> Encoder<'slides> {
    pub fn new(_: &SlideShow<'slides>, _: PagedVec) -> Self {
        todo!()
    }

    /// Encoder part of the file into the paged Vec.
    /// Call it in a loop until done. Full pages will be indicated with `ready` and they might need
    /// to be consumed before we can continue (to not blow memory budget).
    pub fn step(&mut self) {
        todo!()
    }

    /// All full pages of memory of completed file.
    pub fn ready(&self) -> &[[u8; 4096]] {
        todo!()
    }

    /// Tell the encoder that some pages were written to background storage.
    /// Frees up some buffer space for next steps.
    pub fn consume(&mut self, _: usize) {
        todo!()
    }

    pub fn done(&self) -> bool {
        matches!(self.progress, Progress::Done)
    }

    /// All the remaining data after decoding was done.
    pub fn tail(&self) -> &[u8] {
        todo!()
    }
}
