use std::io;
use crate::{Chapter, Slide, Subtitle};
use crate::paged_vec::PagedVec;
use crate::missing_specs::MatroskaSpec as MatroskaSpecExt;

use webm_iterable::{
    WebmWriter,
    matroska_spec::Master,
    matroska_spec::MatroskaSpec,
};

use image::{
    io::Reader as ImageReader,
    ImageError,
    GenericImageView,
};

pub enum Color {
    Srgb,
}

pub enum Audio {
    Pcm {
        sampling_frequency: u32,
        channels: u16,
        bits_per_sample: u16,
    }
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
    state: EncoderState,
}

#[derive(Default)]
struct EncoderState {
    passed_time: f32,
}

/// Internal state keeping.
enum Progress {
    Initial,
    BeforeFrame(usize),
    Done,
}

#[derive(Debug)]
pub struct Error {
    inner: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    Image(ImageError),
    Wav(std::io::Error),
    MismatchingDimensions,
    EmptySequence,
}

/// The 'interface' of how we chose to encode audio.
///
/// That is, the Matroska mapped version that we computed from the input parameters—the string
/// name, the sampling frequency, the track ID etc.
struct AudioTrack {
    /// Sampling frequency for the EBML `Audio.SamplingFrequency` field.
    /// Also calculating an exact timestamp of a chunk after a number of samples.
    sampling_frequency: u32,
    /// Number of channels for the EBML `Audio.Channels` field.
    channels: u16,
    /// Number of channels for the EBML `Audio.BitDepth` field.
    bits_per_sample: u16,
}

/// One block of PCM data.
/// Most of the data that usually occurs in the .wav/riff header and block header is out-of-band in
/// some of the tags of the track, or in the tags of the cluster. Notably we calculate a new audio
/// offset based on the index of the first sample in this slice and the original sample rate.
struct PcmSlice<'data> {
    /// The offset of the data (in time scaling).
    offset: u64,
    /// The raw data to be put into the block.
    data: &'data [u8],
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
    const TIMESCALE: u32 = 1_000_000;
    const APP_NAME: &'static str = "VFP-Core-1.0.0";

    const TRACK_VIDEO: u64 = 1;
    const TRACK_AUDIO: u64 = 2;

    pub fn new(show: &SlideShow<'slides>, vec: PagedVec) -> Self {
        let audio = match show.audio {
            Audio::Pcm { sampling_frequency, channels, bits_per_sample } => {
                AudioTrack { sampling_frequency, channels, bits_per_sample }
            }
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
            state: EncoderState::default(),
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
                self.progress = Progress::BeforeFrame(0);
            },
            Progress::BeforeFrame(frame) => {
                let seconds = self.slides[frame].seconds;
                let pastframe = self.state.passed_time + seconds;

                match self.encode_audio_chunks(frame) {
                    Ok(Ok(_)) => {},
                    other => return other,
                }

                let mut passed = 0.0;
                while {
                    let length = (seconds - passed).min(1.0);
                    match self.encode_frame_with_duration(frame, Some(length))? {
                        Ok(()) => {},
                        Err(other) => return Ok(Err(other)),
                    }
                    self.state.passed_time += length;
                    passed += length;
                    passed + 0.1 < seconds
                }{}

                self.state.passed_time = pastframe;
                self.state.passed_time += 0.1;

                let next_frame = frame + 1;

                if next_frame == self.slides.len() {
                    // Not sure why, but encode the last frame twice more with no duration.
                    // For some reason this is required to make it show the intended time?
                    // Just going off vlc here and it should not have any impact according
                    // to my understanding of clusters and timecodes. But I did not find
                    // a specification for V_UNCOMPRESSED that went beyond: has raw pixel
                    // data for all frames.
                    let _ = self.encode_frame_with_duration(frame, Some(0.0))?;
                    self.encode_cluster_end();
                    self.progress = Progress::Done;
                } else {
                    self.progress = Progress::BeforeFrame(next_frame);
                }
            }
        }

        Ok(Ok(()))
    }

    /// All full pages of memory of completed file.
    pub fn ready(&self) -> impl std::ops::Deref<Target=[u8]> + '_ {
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
    pub fn tail(&self) -> impl std::ops::Deref<Target=[u8]> + '_ {
        self.vec.ready()
    }

    fn encode_info(&mut self) {
        let _ = self.writer().write(
            &MatroskaSpec::Ebml(Master::Start));
        let _ = self.writer().write(
            &MatroskaSpecExt::EbmlVersion(1));
        let _ = self.writer().write(
            &MatroskaSpecExt::EbmlReadVersion(1));
        let _ = self.writer().write(
            &MatroskaSpec::EbmlMaxIdLength(4));
        let _ = self.writer().write(
            &MatroskaSpec::EbmlMaxSizeLength(8));
        let _ = self.writer().write(
            &MatroskaSpecExt::EbmlDocType("webm".into()));
        let _ = self.writer().write(
            &MatroskaSpecExt::EbmlDocTypeVersion(4));
        let _ = self.writer().write(
            &MatroskaSpecExt::EbmlDocTypeReadVersion(2));
        let _ = self.writer().write(
            &MatroskaSpec::Ebml(Master::End));

        let _ = self.writer().write(
            &MatroskaSpec::Segment(Master::Start));
        let _ = self.writer().write(
            &MatroskaSpec::Info(Master::Start));
        let _ = self.writer().write(
            &MatroskaSpec::TimecodeScale(Self::TIMESCALE.into()));
        let _ = self.writer().write(
            &MatroskaSpec::MuxingApp(Self::APP_NAME.into()));
        let _ = self.writer().write(
            &MatroskaSpec::WritingApp(Self::APP_NAME.into()));
        let total = self.slides
            .iter()
            .fold(0.0, |ts, slide| ts + slide.seconds);
        let total_ns = f64::from(total) * 1_000_000_000f64
            / f64::from(Self::TIMESCALE);
        let _ = self.writer().write(
            &MatroskaSpec::Duration(total_ns));
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
            &MatroskaSpec::TrackNumber(Self::TRACK_VIDEO));
        let uid = self.mk_track_uid(Self::TRACK_VIDEO);
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
            &MatroskaSpec::CodecId("V_UNCOMPRESSED".into()));
        let _ = self.writer().write(
            &MatroskaSpec::CodecDecodeAll(0));
        let _ = self.writer().write(
            &MatroskaSpec::SeekPreRoll(0));
        let _ = self.writer().write(
            &MatroskaSpec::DefaultDuration(1_000_000_000));
        self.video.encode(self.vec.writer());
        let _ = self.writer().write(
            &MatroskaSpec::TrackEntry(Master::End));

        // Audio track:
        let _ = self.writer().write(
            &MatroskaSpec::TrackEntry(Master::Start));
        let _ = self.writer().write(
            &MatroskaSpec::TrackNumber(Self::TRACK_AUDIO));
        let uid = self.mk_track_uid(Self::TRACK_AUDIO);
        let _ = self.writer().write(
            &MatroskaSpec::TrackUid(uid));
        let _ = self.writer().write(
            &MatroskaSpec::TrackType(2));
        let _ = self.writer().write(
            &MatroskaSpec::FlagEnabled(1));
        let _ = self.writer().write(
            &MatroskaSpec::FlagDefault(1));
        let _ = self.writer().write(
            &MatroskaSpec::FlagForced(0));
        let _ = self.writer().write(
            &MatroskaSpec::FlagLacing(0));
        let codec_id = self.audio.pcm_format();
        let _ = self.writer().write(
            &MatroskaSpec::CodecId(codec_id));
        let _ = self.writer().write(
            &MatroskaSpec::CodecDecodeAll(0));
        let _ = self.writer().write(
            &MatroskaSpec::SeekPreRoll(0));
        let _ = self.writer().write(
            &MatroskaSpec::DefaultDuration(1_000_000_000));
        self.audio.encode(self.vec.writer());
        let _ = self.writer().write(
            &MatroskaSpec::TrackEntry(Master::End));

        let _ = self.writer().write(
            &MatroskaSpec::Tracks(Master::End));
    }

    fn encode_chapters(&mut self) {
        // FIXME: let's not worry about it for now.
    }

    fn encode_cluster_end(&mut self) {
        let _ = self.writer().write(
            &MatroskaSpec::Cues(Master::Start));
        let _ = self.writer().write(
            &MatroskaSpec::Cues(Master::End));
        let _ = self.writer().write(
            &MatroskaSpec::Segment(Master::End));
    }

    /// Encode a single frame with our coded (V_UNCOMPRESSED).
    /// This can fail if the new frame is not the correct width/height, it's corrupt, or if the IO
    /// for loading the frame fails.
    #[allow(dead_code)]
    fn encode_frame(&mut self, idx: usize)
        -> Result<Result<(), Error>, io::Error>
    {
        self.encode_frame_with_duration(idx, None)
    }

    fn encode_frame_with_duration(&mut self, idx: usize, duration: Option<f32>)
        -> Result<Result<(), Error>, io::Error>
    {
        let ref frame = self.slides[idx];
        let image = ImageReader::open(&frame.image)?;
        let image = match image.decode() {
            Err(ImageError::IoError(io)) => return Err(io),
            // FIXME: return error data?
            Err(err) => return Ok(Err(err.into())),
            Ok(image) => image,
        };

        if image.dimensions() != (self.video.width, self.video.height) {
            return Ok(Err(ErrorKind::MismatchingDimensions.into()));
        }

        // The data, note that we encode the timestamp in the cluster.
        let data = self.build_frame_block(Self::TRACK_VIDEO, 0i16, image);

        let _ = self.writer().write(
            &MatroskaSpec::Cluster(Master::Start));
        let ts = self.time_as_timecode(self.state.passed_time);
        let _ = self.writer().write(
            &MatroskaSpec::Timecode(ts));
        let duration = duration.unwrap_or(frame.seconds);
        let duration = self.time_as_timecode(duration);
        let _ = self.writer().write(
            &MatroskaSpec::BlockGroup(Master::Start));
        let _ = self.writer().write_raw(
            0xa1, &data[..]);
        let _ = self.writer().write(
            &MatroskaSpec::BlockDuration(duration));
        let _ = self.writer().write(
            &MatroskaSpec::BlockGroup(Master::End));
        let _ = self.writer().write(
            &MatroskaSpec::Cluster(Master::End));

        Ok(Ok(()))
    }

    fn encode_audio_chunks(&mut self, idx: usize)
        -> Result<Result<(), Error>, io::Error>
    {
        let ref frame = self.slides[idx];
        let mut audiofile;
        let audiofile = wav::read({
            audiofile = std::fs::File::open(&frame.audio)?;
            &mut audiofile
        });

        // FIXME: should check that the wav data kind matches the header.
        let (_, mut data) = match super::convert_wav_result(audiofile) {
            Ok(items) => items,
            Err(other) => return other.map(Err),
        };

        for pcm in self.audio.pcm_chunk(&mut data) {
            let data = self.build_pcm_block(Self::TRACK_AUDIO, 0i16, pcm.data);

            let _ = self.writer().write(
                &MatroskaSpec::Cluster(Master::Start));
            let ts = self.time_as_timecode(self.state.passed_time);
            let ts = ts + pcm.offset;
            let _ = self.writer().write(
                &MatroskaSpec::Timecode(ts));
            let _ = self.writer().write(
                &MatroskaSpec::BlockGroup(Master::Start));
            let _ = self.writer().write_raw(
                0xa1, &data[..]);
            let _ = self.writer().write(
                &MatroskaSpec::BlockGroup(Master::End));
            let _ = self.writer().write(
                &MatroskaSpec::Cluster(Master::End));
        }

        Ok(Ok(()))
    }

    fn build_block(&self, num: u64, ts: i16, data: &[u8]) -> Vec<u8> {
        assert!(num < 0x80, "Multi-byte track number not implemented");
        let num = (num | 0x80) as u8;
        let [ts0, ts1] = ts.to_be_bytes();
        let mut vec = vec![num, ts0, ts1, 0x00];
        vec.extend_from_slice(data);
        vec
    }

    fn build_frame_block(&self, num: u64, ts: i16, frame: image::DynamicImage)
        -> Vec<u8>
    {
        let frame = image::DynamicImage::ImageRgba8(frame.to_rgba8());
        self.build_block(num, ts, frame.as_bytes())
    }

    fn build_pcm_block(&self, num: u64, ts: i16, data: &[u8])
        -> Vec<u8>
    {
        self.build_block(num, ts, data)
    }

    fn time_as_timecode(&self, secs: f32) -> u64 {
        self.time_as_timecode_f64(f64::from(secs))
    }

    fn time_as_timecode_f64(&self, secs: f64) -> u64 {
        let ts = secs * f64::from(1_000_000_000) / f64::from(Self::TIMESCALE);
        ts.round() as u64
    }

    fn writer(&mut self) -> &'_ mut WebmWriter<impl io::Write> {
        self.vec.writer()
    }

    fn mk_track_uid(&self, id: u64) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id.hash(&mut hasher);
        hasher.finish()
    }
}

impl VideoTrack {
    fn encode(&self, writer: &mut WebmWriter<impl io::Write>) {
        let _ = writer.write(
            &MatroskaSpec::Video(Master::Start));
        let _ = writer.write(
            &MatroskaSpec::FlagInterlaced(2));
        let _ = writer.write(
            &MatroskaSpec::PixelWidth(self.width.into()));
        let _ = writer.write(
            &MatroskaSpec::PixelHeight(self.height.into()));
        let _ = writer.write(
            &MatroskaSpec::ColourSpace(b"RGBA".to_vec()));
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

impl AudioTrack {
    fn encode(&self, writer: &mut WebmWriter<impl io::Write>) {
        let _ = writer.write(
            &MatroskaSpec::Audio(Master::Start));
        let _ = writer.write(
            &MatroskaSpec::SamplingFrequency(self.sampling_frequency.into()));
        let _ = writer.write(
            &MatroskaSpec::Channels(self.channels.into()));
        let _ = writer.write(
            &MatroskaSpec::BitDepth(self.bits_per_sample.into()));
        let _ = writer.write(
            &MatroskaSpec::Audio(Master::End));
    }

    /// The precise, full coded format.
    fn pcm_format(&self) -> String {
        match 1u16.to_be_bytes() == 1u16.to_ne_bytes() {
            true => "A_PCM/INT/BIG".into(),
            false => "A_PCM/INT/LIT".into(),
        }
    }

    fn float_format(&self) -> String {
        "A_PCM/FLOAT/IEEE".into()
    }

    /// Get one chunk of data for this cluster.
    fn pcm_chunk<'data>(&self, data: &'data mut wav::BitDepth)
        -> impl Iterator<Item=PcmSlice<'data>> + 'data
    {
        let chunk_len_ms = 33;
        let sampling_frequency = self.sampling_frequency;
        let count = (sampling_frequency / chunk_len_ms) as usize;

        let chunks: Box<dyn Iterator<Item=&[u8]>> = match data {
            wav::BitDepth::Empty => unreachable!(),
            wav::BitDepth::Eight(data) => {
                Box::new(data.chunks(count))
            },
            wav::BitDepth::Sixteen(data) => {
                Box::new(data.chunks(count).map(bytemuck::cast_slice))
            },
            wav::BitDepth::TwentyFour(_) => {
                unimplemented!("Not contiguous in memory, need different strategy.")
            },
            wav::BitDepth::ThirtyTwoFloat(data) => {
                // We MUST use little endian here.
                let raw = bytemuck::cast_slice_mut::<_, [u8; 4]>(data);

                raw
                    .iter_mut()
                    .for_each(|val| {
                        *val = u32::from_ne_bytes(*val).to_ne_bytes();
                    });

                Box::new(data.chunks(count).map(bytemuck::cast_slice))
            },
        };

        chunks
            .scan(0, move |num, data| {
                let secs = (*num) as f64 / f64::from(sampling_frequency);
                let offset = secs * f64::from(1_000_000_000) / f64::from(Encoder::TIMESCALE);
                *num += count;

                Some(PcmSlice {
                    offset: offset as u64,
                    data,
                })
            })
    }
}

impl From<ErrorKind> for Error {
    fn from(inner: ErrorKind) -> Self {
        Error { inner }
    }
}

impl From<ImageError> for Error {
    fn from(err: ImageError) -> Self {
        Error { inner: ErrorKind::Image(err) }
    }
}
