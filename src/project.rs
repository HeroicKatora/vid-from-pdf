use std::{io, fs, path::PathBuf};
use index_ext::Int;
use serde::{Serialize, Deserialize};

use crate::FatalError;
use crate::app::App;
use crate::ffmpeg::Assembly;
use crate::sink::{FileSource, Identifier, Sink, Source};

/// A video project.
///
/// We must have one particular pdf as the source.
pub struct Project {
    pub dir: Sink,
    pub project_id: Identifier,
    pub meta: Meta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meta {
    pub source: PathBuf,
    pub slides: Vec<Slide>,
    pub ffcontrol: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub replacement: Replacement,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Slide {
    pub visual: Visual,
    pub audio: Option<PathBuf>,
    /// The visual, converted to PNG.
    pub png: Option<PathBuf>,
    /// The visual, converted to SVG.
    pub svg: Option<PathBuf>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Replacement {
    pub path: Option<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Visual {
    /// A particular slide.
    Slide {
        src: PathBuf,
        idx: usize,
    },
    // TODO: replacement image?
    // TODO: or continue last frame?
    // TODO: movies? It would be 'free'.
}

impl Project {
    /// FIXME: async.
    pub fn new(
        in_dir: &mut Sink,
        from: &mut dyn io::BufRead,
    ) -> Result<Self, FatalError> {
        let unique = in_dir.unique_mkdir()?;
        let mut sink = Sink::new(unique.path)?;

        let meta = Meta {
            source: sink.store_to_file(from)?,
            slides: vec![],
            ffcontrol: None,
            output: None,
            replacement: Replacement::default(),
        };

        let project = Project {
            dir: sink,
            project_id: unique.identifier,
            meta,
        };

        project.store()?;
        Ok(project)
    }

    /// Open an existing directory as a project.
    pub fn load(
        app: &App,
        project_id: Identifier,
    ) -> Result<Option<Self>, FatalError> {
        let sink = app.sink.as_sink();
        let unique_path = sink.path_of(project_id);

        if !unique_path.exists() {
            return Ok(None);
        }

        let sink = Sink::new(unique_path)?;
        let meta = {
            use io::Read;
            // TODO: cap read at some limit here?
            let file = sink.work_dir().join(Self::PROJECT_META);
            let meta = fs::File::open(file)?;
            let mut data = vec![];
            let max_len = app.limits.meta_size();
            meta.take(max_len).read_to_end(&mut data)?;

            if data.get_int(..max_len).is_some() {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "excessive project meta data file",
                ))?;
            }

            serde_json::from_slice(data.as_slice())
                .map_err(FatalError::Corrupt)?
        };

        Ok(Some(Project {
            dir: sink,
            project_id,
            meta,
        }))
    }

    pub fn import_audio(&mut self, idx: usize, file: &mut impl Source) -> Result<(), FatalError> {
        let path = self.dir.store_to_file(file.as_buf_read())?;
        self.meta.slides[idx].audio = Some(path);
        Ok(())
    }

    // FIXME: not fatal errors, such as missing information.
    pub fn assemble(&mut self, app: &App) -> Result<(), FatalError> {
        let mut assembly = Assembly::new(&mut self.dir)?;

        for slide in &mut self.meta.slides {
            let visual = slide.render_visual(&mut self.dir, app)?;
            let audio = match &slide.audio {
                None => {
                    let path = self.meta.replacement.silent_audio(&mut self.dir, app)?;
                    FileSource::new_from_existing(path.clone())?
                },
                Some(path) => FileSource::new_from_existing(path.clone())?,
            };
            assembly.add_linked(&app.ffmpeg, &visual, &audio, &mut self.dir)?;
        }

        let mut outsink = &mut self.dir;
        assembly.finalize(&app.ffmpeg, &mut outsink)?;

        let output = outsink
            .imported()
            .next()
            .ok_or_else(|| FatalError::Io(io::Error::new(
                io::ErrorKind::NotFound.into(),
                "Apparently no output was produced",
            )))?;

        self.meta.output = Some(output);
        Ok(())
    }

    /// Convert all visuals to png versions.
    pub fn thumbnail(&mut self) -> Result<(), FatalError> {
        for slide in &mut self.meta.slides {
            match slide.visual {
                Visual::Slide { ref src, .. } => {
                    let mut path = src.clone();
                    path.set_extension("svg");
                    fs::copy(src, &path)?;
                    slide.svg = Some(path);
                }
            }
        }
        Ok(())
    }

    pub fn store(&self) -> Result<(), FatalError> {
        let file = self.dir.work_dir().join(Self::PROJECT_META);
        let meta = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(file)?;
        serde_json::to_writer(meta, &self.meta).map_err(io::Error::from)?;
        Ok(())
    }

    pub fn explode(&mut self, app: &App) -> Result<(), FatalError> {
        let mut source = FileSource::new_from_existing(self.meta.source.clone())?;
        app.explode.explode(&mut source, &mut self.dir)?;

        self.meta.slides.clear();
        for (idx, src) in self.dir.imported().enumerate() {
            self.meta.slides.push(Slide {
                visual: Visual::Slide { src, idx, },
                audio: None,
                png: None,
                svg: None,
            })
        }

        Ok(())
    }

    const PROJECT_META: &'static str = ".project";
}

impl Slide {
    fn render_visual(&mut self, sink: &mut Sink, app: &App) -> Result<FileSource, FatalError> {
        // Shortcut, if we already have a pixmap.
        if let Some(src) = &self.png {
            let file_source = FileSource::new_from_existing(src.clone())?;
            return Ok(file_source);
        }

        match &self.visual {
            Visual::Slide { src, .. } => {
                let mut path = src.clone();
                // usvg is picky about file endings. GEEEEEEEZ.
                path.set_extension("svg");
                fs::copy(src, &path)?;
                self.svg = Some(path);
                let path = self.svg.as_ref().unwrap();

                let svg = app.magick.open(path)?;
                let image = svg.render()?;
                let unique = sink.unique_path()?;
                let image = image.to_rgba8();

                image.save_with_format(&unique.path, image::ImageFormat::Png)?;
                self.png = Some(unique.path);
            },
        }

        if let Some(png) = &self.png {
            let file_source = FileSource::new_from_existing(png.clone())?;
            Ok(file_source)
        } else {
            Err(FatalError::UnrecognizedInputSlide)
        }
    }
}

impl Replacement {
    fn silent_audio(&mut self, sink: &mut Sink, app: &App) -> Result<&PathBuf, FatalError> {
        if self.path.is_none() {
            app.ffmpeg.replacement_audio(10.0f32, sink)?;
            let file = sink
                .imported()
                .next()
                .ok_or_else(|| io::Error::new(
                    io::ErrorKind::NotFound,
                    "ffmpeg failed to produce replacement audio",
                ))?;
            self.path = Some(file);
        }

        Ok(self.path.as_ref().unwrap())
    }

    fn convert_ppm_to_png(&self, sink: &mut Sink, path: &PathBuf) -> Result<PathBuf, FatalError> {
        let image = {
            let mut reader = image::io::Reader::open(path)?;
            reader.set_format(image::ImageFormat::Pnm);
            reader.decode()?
        };

        let unique = sink.unique_path()?;
        image.save_with_format(&unique.path, image::ImageFormat::Png)?;
        Ok(unique.path)
    }
}
