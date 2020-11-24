use std::{fmt, fs, io, process::Command, process::Stdio, path::PathBuf};
use libloading::{Library, Symbol,library_filename};
use which::CanonicalPath;

use crate::FatalError;
use crate::sink::{FileSource, Sink};
use crate::resources::{RequiredToolError, require_tool};

pub struct Ffmpeg {
    /// The main ffmpeg executable.
    pub ffmpeg: CanonicalPath,
    /// The main ffprobe executable.
    pub ffprobe: CanonicalPath,
    /// Proof type that we understand the versioning.
    /// Also extension if we ever care about loading the configuration, inspecting details of
    /// libavutils and plugins, etc.
    pub version: Version,
    /// The hardware acceleration to use.
    pub hw_accel: HwAccelFlavor,
}

#[derive(Clone, Copy)]
pub enum HwAccelFlavor {
    None,
    NvEnc,
    VdPau,
}

pub struct Assembly {
    video_list: fs::File,
    video_path: PathBuf,
    audio_list: fs::File,
    audio_path: PathBuf,
    slide_list: Vec<(PathBuf, f32)>,
}

pub struct Version {
    pub version: versions::Version,
}

pub enum LoadFfmpegError {
    CantFindTool(RequiredToolError),
    VersionNumberIsGibberish,
    VersionNumberIsUnrecognized(String),
}

impl Ffmpeg {
    pub fn new() -> Result<Ffmpeg, LoadFfmpegError> {
        let ffprobe = require_tool("ffprobe")?;
        let ffmpeg = require_tool("ffmpeg")?;

        let hw_accel = Self::detect_hardware_accel();

        // TODO: minimum version requirements?
        let version = Command::new(&ffmpeg)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .arg("-version")
            .output()
            .map_err(LoadFfmpegError::io_error)
            .and_then(parse_version)?;

        // We don't really care for version. ffprobe should be distributed with ffmpeg so let's
        // assume that if it is present then it is generally the same.
        match {
            Command::new(&ffprobe)
                .arg("-version")
                // do not inherit any input or output
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .status()
                .map(|status| status.success())
        } {
            Ok(true) => {},
            Ok(false) => return Err(LoadFfmpegError::VersionNumberIsGibberish),
            Err(err) => return Err(LoadFfmpegError::io_error(err)),
        }

        Ok(Ffmpeg {
            ffmpeg,
            ffprobe,
            version,
            hw_accel,
        })
    }

    fn detect_hardware_accel() -> HwAccelFlavor {
        let filename = library_filename("avcodec");
        let library = match Library::new(filename) {
            Ok(lib) => lib,
            Err(_) => return HwAccelFlavor::None,
        };

        type Type = unsafe fn(name: *const std::os::raw::c_char) -> *const std::os::raw::c_void;
        // SAFETY: that's a function pointer according to libavcodec.
        // We also don't leak that pointer statically as the type would technically permit.
        let avcodec_find_encoder_by_name: Symbol<Type> = unsafe {
            match library.get(b"avcodec_find_encoder_by_name\0") {
                Ok(fn_symbol) => fn_symbol,
                Err(_) => return HwAccelFlavor::None,
            }
        };

        const NVENC_NAME: &'static str = "h264_nvenc\0";
        const VDPAU_NAME: &'static str = "h264_vdpau\0";

        if !unsafe { avcodec_find_encoder_by_name(NVENC_NAME.as_ptr() as *const _) }.is_null() {
            return HwAccelFlavor::NvEnc;
        }

        if !unsafe { avcodec_find_encoder_by_name(VDPAU_NAME.as_ptr() as *const _) }.is_null() {
            return HwAccelFlavor::VdPau;
        }

        return HwAccelFlavor::None;
    }

    /// Determine the duration of an audio file with ffmpeg tools.
    pub fn audio_duration(&self, file: &FileSource, sink: &mut Sink) -> Result<f32, FatalError> {
        // TODO: might be more convenient to have another error type here.
        let output = Command::new(self.ffprobe.as_path())
            .current_dir(sink.work_dir())
            .args(&["-v", "error"])
            .args(&["-show_entries", "format=duration"])
            .args(&["-of", "default=noprint_wrappers=1:nokey=1"])
            .arg(file.as_path())
            .output()?;

        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = output.stderr;

        let duration: f32 = stdout
            .trim()
            .parse()
            .map_err(|err| {
                eprintln!("{}", &stdout);
                eprintln!("{}", String::from_utf8_lossy(&stderr));
                io::Error::new(io::ErrorKind::InvalidData, err)
            })?;
        Ok(duration)
    }

    pub fn replacement_audio(&self, duration: f32, sink: &mut Sink) -> Result<(), FatalError> {
        let duration = duration.to_string();
        let unique = sink.unique_path()?;

        let success = Command::new(self.ffmpeg.as_path())
            .current_dir(sink.work_dir())
            .args(&["-f", "lavfi", "-i", "anullsrc=r=11025:cl=mono", "-t"])
            .arg(duration)
            .args(&["-f", "wav"])
            .arg(&unique.path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .status()
            .map(|status| status.success())?;
        
        if !success {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "ffmpeg was unable to produce silent audio"
            ).into());
        }

        sink.import(unique.path);
        Ok(())
    }
}

impl Assembly {
    pub fn new(sink: &mut Sink) -> Result<Self, FatalError> {
        let video_ctrl = sink.unique_file(fs::OpenOptions::new().write(true))?;
        let audio_ctrl = sink.unique_file(fs::OpenOptions::new().write(true))?;
        Ok(Assembly {
            audio_list: audio_ctrl.file,
            audio_path: audio_ctrl.path,
            video_list: video_ctrl.file,
            video_path: video_ctrl.path,
            slide_list: vec![],
        })
    }

    pub fn add_linked(
        &mut self,
        ffmpeg: &Ffmpeg,
        visual: &FileSource,
        audio: &FileSource,
        sink: &mut Sink,
    )
        -> Result<(), FatalError>
    {
        use std::io::Write as _;
        let duration = ffmpeg.audio_duration(audio, sink)?;
        self.slide_list.push((visual.as_path().to_owned(), duration));
        writeln!(&self.video_list, "file '{}'", visual.as_path().display()).unwrap();
        writeln!(&self.video_list, "duration {}", duration).unwrap();
        writeln!(&self.audio_list, "file {}", audio.as_path().display())?;
        Ok(())
    }

    // FIXME: this MUST be async or run in another thread.
    pub fn finalize(&self, ffmpeg: &Ffmpeg, sink: &mut Sink) -> Result<(), FatalError> {
        // concatenate all audio
        let mut audio_out = sink.unique_path()?;
        audio_out.path.set_extension("wav");
        let output = Command::new(&ffmpeg.ffmpeg)
            .current_dir(sink.work_dir())
            // ffmpeg rejects paths if any component has a leading `.`. That's pretty stupid for
            // scripting as tempfile does begin all its tempdirs with a literal dot.
            .args(&["-f", "concat", "-safe", "0", "-i"])
            .arg(&self.audio_path)
            .args(&["-c", "copy"])
            .arg(&audio_out.path)
            .output()?;

        if !output.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("{:?}", output),
            ).into());
        }

        let meta = self.create_meta_data(sink)?;

        let mut video_out = sink.unique_path()?;
        video_out.path.set_extension("mp4");
        let hw_encoder = ffmpeg.hw_accel.as_encoder_str();

        // Join audio to concatenated video.
        let output = Command::new(&ffmpeg.ffmpeg)
            .current_dir(sink.work_dir())
            // ffmpeg rejects paths if any component has a leading `.`. That's pretty stupid for
            // scripting as tempfile does begin all its tempdirs with a literal dot.
            .arg("-i")
            .arg(&audio_out.path)
            .args(&["-f", "concat", "-safe", "0", "-i"])
            .arg(&self.video_path)
            .arg("-i")
            .arg(&meta)
            .args(&["-map_metadata", "2"])
            // FIXME: use `h264_nvenc` or `h264_vaapi` where available.
            // Find out how to probe for these.
            .args(&["-c:v", hw_encoder, "-framerate", "2", "-preset", "fast", "-c:a", "aac"])
            .args(&["-vf", "scale=w=1920:h=1080:force_original_aspect_ratio=decrease:flags=lanczos"])
            .arg(&video_out.path)
            .output()?;

        if !output.status.success() {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("{:?}", output),
            ).into());
        }

        sink.import(video_out.path);

        Ok(())
    }

    fn create_meta_data(&self, sink: &mut Sink) -> Result<PathBuf, FatalError> {
        use std::io::Write as _;

        let meta = sink.unique_path()?;
        let meta_file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .open(&meta.path)?;

        writeln!(
            &meta_file,
            ";FFMETADATA1\n\
            title=Created with vid-from-pdf",
        )?;

        let mut up_to_now = 0.0;
        for (idx, (_, ch_len)) in self.slide_list.iter().enumerate() {
            let start = up_to_now;
            up_to_now += ch_len;
            writeln!(
                &meta_file,
                "[CHAPTER]\n\
                TIMEBASE=1/1000\n\
                START={start}\n\
                END={end}\n\
                title=Chapter {chapter_idx}",
                start=(start*1000.0) as u64,
                end=(up_to_now*1000.0) as u64,
                chapter_idx=idx+1,
            )?;
        }

        Ok(meta.path)
    }
}

impl HwAccelFlavor {
    pub fn as_encoder_str(self) -> &'static str {
        match self {
            HwAccelFlavor::None => "libx264",
            HwAccelFlavor::VdPau => "h264_vdpau",
            HwAccelFlavor::NvEnc => "h264_nvenc",
        }
    }
}

impl LoadFfmpegError {
    fn io_error(_: std::io::Error) -> Self {
        // TODO: really? Maybe this should be fatal somehow.
        LoadFfmpegError::VersionNumberIsGibberish
    }
}

fn parse_version(output: std::process::Output) -> Result<Version, LoadFfmpegError> {
    let str_output;
    // ffmpeg version n4.3.1 Copyright (c) 2000-2020 the FFmpeg developers
    let first_line = match {
        str_output = String::from_utf8(output.stdout);
        &str_output
    } {
        Ok(st) => st.lines().next().unwrap(),
        Err(_) => return Err(LoadFfmpegError::VersionNumberIsGibberish),
    };

    let signature = "ffmpeg version ";

    if !first_line.starts_with(signature) {
        return Err(LoadFfmpegError::VersionNumberIsGibberish);
    }

    let (_, version) = first_line.split_at(signature.len());
    let version = version.split_whitespace().next().unwrap();
    let mut chars = version.chars();

    match chars.clone().next() {
        None => return Err(LoadFfmpegError::VersionNumberIsGibberish),
        Some(d) if d.is_digit(10) => {},
        Some(_) => { let _ = chars.next(); }
    };

    let version = match versions::Version::new(chars.as_str()) {
        Some(version) => version,
        None => return Err(LoadFfmpegError::VersionNumberIsUnrecognized(version.to_string())),
    };

    Ok(Version {
        version,
    })
}

impl From<RequiredToolError> for LoadFfmpegError {
    fn from(err: RequiredToolError) -> Self {
        LoadFfmpegError::CantFindTool(err)
    }
}

impl fmt::Display for LoadFfmpegError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadFfmpegError::CantFindTool(err) => err.fmt(f),
            LoadFfmpegError::VersionNumberIsGibberish => {
                write!(f, "The ffmpeg program did not appear to provide version information.")
            }
            LoadFfmpegError::VersionNumberIsUnrecognized(nr) => {
                write!(f, "The ffmpeg program provided version number `{}` but it was not understood.", nr)
            }
        }
    }
}
