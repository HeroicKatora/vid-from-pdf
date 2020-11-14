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
        };

        // TODO: store meta to disk.

        Ok(Project {
            dir: sink,
            project_id: unique.identifier,
            meta,
        })
    }

    /// Open an existing directory as a project.
    pub fn load(
        app: &App,
        in_dir: PathBuf,
        project_id: Identifier,
    ) -> Result<Option<Self>, FatalError> {
        let sink = Sink::new(in_dir)?;
        let unique_path = sink.path_of(project_id);

        if !unique_path.exists() {
            return Ok(None);
        }

        let sink = Sink::new(unique_path)?;
        let meta = {
            use io::Read;
            // TODO: cap read at some limit here?
            let meta = fs::File::open(sink.work_dir().join(".project"))?;
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
}
