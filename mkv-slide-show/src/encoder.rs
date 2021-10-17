use std::io;
use crate::{Chapter, Slide, Subtitle};
use crate::paged_vec::PagedVec;
use crate::missing_specs::MatroskaSpec as MatroskaSpecExt;

use webm_iterable::{
    WebmWriter,
    matroska_spec::Master,
    matroska_spec::MatroskaSpec,
};

pub enum Color {
    Srgb,
}

pub enum Audio {
    Raw16BitLE,
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
    vec: PagedVec,
}

/// Internal state keeping.
enum Progress {
    Initial,
    BeforeFrame(usize),
    Done,
}

#[derive(Debug)]
pub struct Error;

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
    width: u32,
    height: u32,
}

/// See mandatory and optional fields:
/// <https://www.matroska.org/technical/elements.html>
impl<'slides> Encoder<'slides> {
    pub fn new(show: &SlideShow<'slides>, vec: PagedVec) -> Self {
        let audio = AudioTrack {
        };

        let video = VideoTrack {
            width: show.width,
            height: show.height,
        };

        Encoder {
            slides: show.slides,
            audio,
            video,
            progress: Progress::Initial,
            vec,
        }
    }

    /// Encoder part of the file into the paged Vec.
    /// Call it in a loop until done. Full pages will be indicated with `ready` and they might need
    /// to be consumed before we can continue (to not blow memory budget).
    pub fn step(&mut self) -> Result<Result<(), Error>, io::Error> {
        match self.progress {
            Progress::Done => {},
            Progress::Initial => {
                self.encode_info();
                self.encode_tracks();
                self.encode_chapters();
                self.encode_cluster_head();
                self.progress = Progress::BeforeFrame(0);
            },
            Progress::BeforeFrame(frame) => {
                match self.encode_frame(frame)?{
                    Ok(()) => {},
                    Err(other) => return Ok(Err(other)),
                }

                if frame == self.slides.len() {
                    self.encode_cluster_end();
                    self.progress = Progress::Done;
                }
            }
        }

        Ok(Ok(()))
    }

    /// All full pages of memory of completed file.
    pub fn ready(&self) -> &[u8] {
        self.vec.ready()
    }

    /// Tell the encoder that some pages were written to background storage.
    /// Frees up some buffer space for next steps.
    pub fn consume(&mut self, pages: usize) {
        self.vec.consume(pages)
    }

    pub fn done(&self) -> bool {
        matches!(self.progress, Progress::Done)
    }

    /// All the remaining data after decoding was done.
    pub fn tail(&self) -> &[u8] {
        self.vec.ready()
    }

    fn encode_info(&mut self) {
        let _ = self.writer().write(
            &MatroskaSpec::Segment(Master::Start));
        let _ = self.writer().write(
            &MatroskaSpec::Info(Master::Start));
        let _ = self.writer().write(
            &MatroskaSpec::TimecodeScale(1_000_000));
        let _ = self.writer().write(
            &MatroskaSpec::MuxingApp("VFP-Core-1.0.0".into()));
        let _ = self.writer().write(
            &MatroskaSpec::WritingApp("VFP-Core-1.0.0".into()));
        let _ = self.writer().write(
            &MatroskaSpec::Info(Master::End));
    }

    fn encode_tracks(&mut self) {
        let _ = self.writer().write(
            &MatroskaSpec::Tracks(Master::Start));

        // Video track:
        let _ = self.writer().write(
            &MatroskaSpec::TrackEntry(Master::Start));
        let _ = self.writer().write(
            &MatroskaSpec::TrackNumber(0));
        let uid = self.mk_track_uid();
        let _ = self.writer().write(
            &MatroskaSpec::TrackUid(uid));
        let _ = self.writer().write(
            &MatroskaSpec::TrackType(1));
        let _ = self.writer().write(
            &MatroskaSpec::FlagEnabled(1));
        let _ = self.writer().write(
            &MatroskaSpec::FlagDefault(1));
        let _ = self.writer().write(
            &MatroskaSpec::FlagForced(0));
        let _ = self.writer().write(
            &MatroskaSpec::FlagLacing(0));
        let _ = self.writer().write(
            &MatroskaSpec::MinCache(0));
        let _ = self.writer().write(
            &MatroskaSpec::MaxBlockAdditionId(0));
        let _ = self.writer().write(
            &MatroskaSpec::CodecId("V_UNCOMPRESSED".into()));
        let _ = self.writer().write(
            &MatroskaSpec::CodecDecodeAll(0));
        let _ = self.writer().write(
            &MatroskaSpec::SeekPreRoll(0));
        self.video.encode(self.vec.writer());
        let _ = self.writer().write(
            &MatroskaSpec::TrackEntry(Master::End));

        let _ = self.writer().write(
            &MatroskaSpec::Tracks(Master::End));
    }

    fn encode_chapters(&mut self) {
        // FIXME: let's not worry about it for now.
    }

    fn encode_cluster_head(&mut self) {
        let _ = self.writer().write(
            &MatroskaSpec::Cluster(Master::Start));
    }

    fn encode_cluster_end(&mut self) {
        let _ = self.writer().write(
            &MatroskaSpec::Cluster(Master::End));
        let _ = self.writer().write(
            &MatroskaSpec::Segment(Master::End));
    }

    /// Encode a single frame with our coded (V_UNCOMPRESSED).
    /// This can fail if the new frame is not the correct width/height, it's corrupt, or if the IO
    /// for loading the frame fails.
    fn encode_frame(&mut self, idx: usize)
        -> Result<Result<(), Error>, io::Error>
    {
        let ref frame = self.slides[idx];

        Ok(Ok(()))
    }

    fn writer(&mut self) -> WebmWriter<&'_ mut dyn io::Write> {
        self.vec.writer()
    }

    fn mk_track_uid(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        0u64.hash(&mut hasher);
        hasher.finish()
    }
}

impl VideoTrack {
    fn encode(&self, mut writer: WebmWriter<&'_ mut dyn io::Write>) {
        let _ = writer.write(
            &MatroskaSpec::Video(Master::Start));
        let _ = writer.write(
            &MatroskaSpec::PixelWidth(self.width.into()));
        let _ = writer.write(
            &MatroskaSpec::PixelHeight(self.height.into()));
        let _ = writer.write(
            &MatroskaSpec::ColourSpace(b"RGB2".to_vec()));
        let _ = writer.write(
            &MatroskaSpecExt::Color(Master::Start));
        let _ = writer.write(
            &MatroskaSpecExt::BitsPerChannel(8));
        // sRGB
        let _ = writer.write(
            &MatroskaSpecExt::TransferCharacteristics(13));
        // sRGB (=rec.709)
        let _ = writer.write(
            &MatroskaSpecExt::Primaries(1));
        let _ = writer.write(
            &MatroskaSpecExt::Color(Master::End));
        // FIXME: should write Color::start etc.
        let _ = writer.write(
            &MatroskaSpec::Video(Master::End));
    }
}
