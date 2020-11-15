use std::{fmt, fs, io, process::Command, process::Stdio, path::PathBuf};
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
}

pub struct Assembly {
    video_list: fs::File,
    video_path: PathBuf,
    audio_list: fs::File,
    audio_path: PathBuf,
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
        })
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

        let duration: f32 = String::from_utf8(output.stdout)
            .unwrap()
            .trim()
            .parse()
            .map_err(|err| io::Error::new(
                io::ErrorKind::InvalidData,
                err
            ))?;
        Ok(duration)
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

        let mut video_out = sink.unique_path()?;
        video_out.path.set_extension("mp4");
        // Join audio to concatenated video.
        let output = Command::new(&ffmpeg.ffmpeg)
            .current_dir(sink.work_dir())
            // ffmpeg rejects paths if any component has a leading `.`. That's pretty stupid for
            // scripting as tempfile does begin all its tempdirs with a literal dot.
            .arg("-i")
            .arg(&audio_out.path)
            .args(&["-f", "concat", "-safe", "0", "-i"])
            .arg(&self.video_path)
            .args(&["-filter_complex", r#"[1:v][0:a]concat=n=1:v=1:a=1[sizev][outa];[sizev]scale=ceil(iw/2)*2:ceil(ih/2)*2[outv]"#])
            .args(&["-map", "[outv]", "-map", "[outa]", "-pix_fmt", "yuv420p"])
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
