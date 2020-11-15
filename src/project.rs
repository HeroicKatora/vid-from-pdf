use std::{io, fs, path::PathBuf};
use index_ext::Int;
use serde::{Serialize, Deserialize};

use crate::FatalError;
use crate::app::App;
use crate::ffmpeg::Assembly;
use crate::sink::{FileSource, Identifier, Sink};

/// A video project.
///
/// We must have one particular pdf as the source.
pub struct Project {
    pub dir: Sink,
    pub project_id: Identifier,
    pub meta: Meta,
}

#[derive(Serialize, Deserialize)]
pub struct Meta {
    pub source: PathBuf,
    pub slides: Vec<Slide>,
    pub ffcontrol: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
pub struct Slide {
    pub visual: Visual,
    pub audio: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
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

    // FIXME: not fatal errors, such as missing information.
    pub fn assemble(&mut self, app: &App) -> Result<Assembly, FatalError> {
        let mut assembly = Assembly::new(&mut self.dir)?;
        for slide in &self.meta.slides {
            let visual = match &slide.visual {
                Visual::Slide { src, .. } => FileSource::new_from_existing(src.clone())?,
            };
            let audio = match &slide.audio {
                None => return Err(FatalError::Io(io::ErrorKind::Other.into())),
                Some(path) => FileSource::new_from_existing(path.clone())?,
            };
            assembly.add_linked(&app.ffmpeg, &visual, &audio, &mut self.dir)?;
        }
        Ok(assembly)
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
            })
        }

        Ok(())
    }

    const PROJECT_META: &'static str = ".project";
}
