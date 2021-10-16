//! A glue crate for rendering an svg to a pixmap that can be saved.
use std::{io, fs, fmt, path::Path};

pub struct Svg {
    /// The original data of the svg.
    data: Vec<u8>,
    magick: MagickConvert,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Clone)]
pub struct MagickConvert {
    magick: which::CanonicalPath,
}

#[derive(Debug)]
enum ErrorKind {
    Io(io::Error),
    Image(image::ImageError),
    Popen(subprocess::PopenError),
    Convert {
        status: subprocess::ExitStatus,
        stderr: Vec<u8>,
    },
    RequiredTool {
        tool: &'static str,
        information: Option<Box<str>>,
    },
}

impl Svg {
    pub fn render(&self) -> Result<image::DynamicImage, Error> {
        self.render_convert(&self.magick)
    }

    fn render_convert(&self, magick: &MagickConvert) -> Result<image::DynamicImage, Error> {
        let tree_data = self.data.clone();
        let exec = subprocess::Exec::cmd(&magick.magick)
            .arg("convert")
            .arg("-verbose")
            .arg("svg:-")
            .arg("ppm:-")
            .stdin(tree_data)
            .stdout(subprocess::Redirection::Pipe)
            .stderr(subprocess::Redirection::Pipe)
            .capture()?;

        if !exec.success() {
            return Err(Error {
                kind: ErrorKind::Convert {
                    status: exec.exit_status,
                    stderr: exec.stderr,
                },
            });
        }

        let image_data = io::Cursor::new(exec.stdout);
        let image = image::io::Reader::with_format(image_data, image::ImageFormat::Pnm)
            .decode()?;
        Ok(image)
    }
}

impl MagickConvert {
    pub const MAGICK: &'static str = "magick";

    pub fn new(magick: which::CanonicalPath) -> Result<Self, Error> {
        let formats = subprocess::Exec::cmd(&magick)
            .arg("identify")
            .arg("-list")
            .arg("format")
            .stdin(subprocess::Redirection::None)
            .stdout(subprocess::Redirection::Pipe)
            .stderr(subprocess::Redirection::Pipe)
            // Should we limit the output?
            .capture()?;

        let lines = match String::from_utf8(formats.stdout) {
            Err(_) => return Err(Error {
                kind: ErrorKind::RequiredTool {
                    tool: "convert",
                    information: None,
                },
            }),
            Ok(string) => string,
        };

        if let Some(true) = Self::check_svg_read(&lines) {} else {
            return Err(Error {
                kind: ErrorKind::RequiredTool {
                    tool: "convert",
                    information: Some("SVG read support".into()),
                }
            });
        }

        if let Some(true) = Self::check_ppm_write(&lines) {} else {
            return Err(Error {
                kind: ErrorKind::RequiredTool {
                    tool: "convert",
                    information: Some("PPM write support".into()),
                }
            });
        }

        Ok(MagickConvert {
            magick,
        })
    }

    pub fn path(&self) -> &Path {
        self.magick.as_path()
    }

    pub fn open(&self, path: &Path) -> Result<Svg, Error> {
        let data = fs::read(path)?;
        Ok(Svg {
            data: data,
            magick: self.clone(),
        })
    }

    fn check_svg_read(st: &str) -> Option<bool> {
        Self::check_format_support(st, "SVG", |mode| {
            Some('r') == mode.chars().next()
        })
    }

    fn check_ppm_write(st: &str) -> Option<bool> {
        Self::check_format_support(st, "PPM", |mode| {
            Some('w') == mode.chars().nth(1)
        })
    }

    fn check_format_support(st: &str, format: &str, test_mode: impl Fn(&str) -> bool)
        -> Option<bool>
    {
        let svg_spec = st
            .lines()
            .filter(|line| line.contains(format));

        for line in svg_spec {
            // Each line in the format table is of the form:
            //    Format  Module    Mode  Description
            // Mode is a subset of rw+, where + means multiple images per file.
            let line = line.trim_start();

            if !line.starts_with(format) {
                continue;
            }

            let format = if let Some(pos) = line.find(' ') {
                line[pos..].trim_start()
            } else {
                continue;
            };

            let mode = if let Some(pos) = format.find(' ') {
                format[pos..].trim_start()
            } else {
                continue;
            };

            if test_mode(mode) {
                return Some(true);
            }
        }

        Some(false)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error {
            kind: ErrorKind::Io(err),
        }
    }
}

impl From<image::ImageError> for Error {
    fn from(err: image::ImageError) -> Self {
        Error {
            kind: ErrorKind::Image(err),
        }
    }
}

impl From<subprocess::PopenError> for Error {
    fn from(err: subprocess::PopenError) -> Self {
        Error {
            kind: ErrorKind::Popen(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ErrorKind::Io(err) => {
                write!(f, "I/O error: {}", err)
            }
            ErrorKind::Image(err) => write!(f, "Image could not be processed: {}", err),
            // TODO: we're missing context here..
            ErrorKind::Popen(err) => write!(f, "Call to subprocess failed: {}", err),
            ErrorKind::Convert { status, stderr } => write!(
                f,
                "Call to `convert` tool failed:\nexit status:{:?}\nstderr\n{}",
                status,
                String::from_utf8_lossy(stderr),
            ),
            ErrorKind::RequiredTool { tool, information: None } => write!(
                f, 
                "Required tool for SVG conversion not found: {}", 
                tool,
            ),
            ErrorKind::RequiredTool { tool, information: Some(info) } => write!(
                f, 
                "Required tool for SVG conversion not found: {}\n{}", 
                tool,
                info,
            ),
        }
    }
}

#[test]
fn simple() {
    use image::GenericImageView;

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/test.svg");
    let magic = which::CanonicalPath::new("magick")
        .expect("Magick convert not found");
    let convert = MagickConvert::new(magic)
        .expect("Magick does not support required format.");

    let svg = convert.open(Path::new(path))
        .expect("Failed to read example svg");
    let image = svg.render()
        .expect("Failed to render");
    assert_eq!(image.width(), 1920);
    assert_eq!(image.height(), 1440);
}
