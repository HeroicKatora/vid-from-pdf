use std::{fmt, process::Command};
use which::CanonicalPath;

use crate::resources::{RequiredToolError, require_tool};

pub struct Ffmpeg {
    /// The main ffmpeg executable.
    ffmpeg: CanonicalPath,
    /// The main ffprobe executable.
    ffprobe: CanonicalPath,
    /// Proof type that we understand the versioning.
    /// Also extension if we ever care about loading the configuration, inspecting details of
    /// libavutils and plugins, etc.
    version: Version,
}

pub struct Version {
    version: versions::Version,
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
            .arg("-version")
            .output()
            .map_err(LoadFfmpegError::io_error)
            .and_then(parse_version)?;

        // We don't really care for version. ffprobe should be distributed with ffmpeg so let's
        // assume that if it is present then it is generally the same.
        let has_ffprobe = Command::new(&ffprobe)
            .arg("-version")
            .status()
            .map(|status| status.success())
            .unwrap_or(false);

        Ok(Ffmpeg {
            ffmpeg,
            ffprobe,
            version,
        })
    }
}

impl LoadFfmpegError {
    fn io_error(err: std::io::Error) -> Self {
        todo!()
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
