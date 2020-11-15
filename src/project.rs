use std::{io, fs, path::PathBuf};
use index_ext::Int;
use serde::{Serialize, Deserialize};

use crate::FatalError;
use crate::app::App;
use crate::sink::{Sink, Identifier};

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
}

#[derive(Serialize, Deserialize)]
pub struct Slide {
    pub visual: PathBuf,
    pub audio: PathBuf,
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

    pub fn store(&self) -> Result<(), FatalError> {
        let file = self.dir.work_dir().join(Self::PROJECT_META);
        let meta = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(file)?;
        serde_json::to_writer(meta, &self.meta).map_err(io::Error::from)?;
        Ok(())
    }

    const PROJECT_META: &'static str = ".project";
}
