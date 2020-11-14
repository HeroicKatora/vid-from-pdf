use std::{io, fs, path::PathBuf};

use crate::FatalError;
use crate::sink::{Sink, Identifier};

/// A video project.
///
/// We must have one particular pdf as the source.
pub struct Project {
    pub dir: Sink,
    pub project_id: Identifier,
    pub meta: Meta,
}

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
        in_dir: PathBuf,
        project_id: Identifier,
    ) -> Result<Option<Self>, FatalError> {
        let sink = Sink::new(in_dir)?;
        let unique_path = sink.path_of(project_id);

        if !unique_path.exists() {
            return Ok(None);
        }

        let sink = Sink::new(unique_path)?;
        let meta = todo!();

        Ok(Some(Project {
            dir: sink,
            project_id,
            meta,
        }))
    }
}
