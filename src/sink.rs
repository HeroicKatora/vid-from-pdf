/// Some owned tempdir or memory controller.
/// Basically, make it somewhat unlikely we clobber anything that other programs want to write or
/// control memory where we can enforce maximum usage. Give us a convenient interface around
/// requesting that particular paths may be kept freed for us. Provide a collector for collecting
/// output as files or in memory and transparently paged?
use std::{fs, io, path::Path, path::PathBuf};
use rand::{rngs::ThreadRng, Rng as _};

use crate::FatalError;

/// See module description.
///
/// TODO: prefix for non-colliding output bunch.
/// TODO: suffix control for store_to_file.
pub struct Sink {
    tempdir: PathBuf,
    trng: ThreadRng,
    /// A temporary storage for outputs of intermediate steps.
    imported: Vec<PathBuf>,
}

#[derive(Clone)]
pub struct SyncSink {
    path: PathBuf,
}

/// A path and its unique identifier.
pub struct UniquePath {
    /// Fully qualified directory for the project.
    pub path: PathBuf,
    /// Identifier for that project.
    pub identifier: Identifier,
}

pub type Identifier = [u8; 16];

impl Sink {
    pub fn new(path: PathBuf) -> Result<Self, FatalError> {
        if {
            let metadata = fs::metadata(&path);
            !metadata.map_or(false, |md| md.is_dir())
        } {
            return Err(FatalError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Expected a directory but couldn't identify it as such {}", path.display())
            )))
        }

        Ok(Sink {
            tempdir: path,
            trng: rand::thread_rng(),
            imported: vec![],
        })
    }

    pub fn path_of(&self, id: Identifier) -> PathBuf {
        const SRC: &'static str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
        assert_eq!(SRC.len(), 64);

        let mut path = String::new();
        for &b in &id {
            let ch = SRC.chars().nth(usize::from(b & 63)).unwrap();
            path.push(ch);
        }

        self.tempdir.join(&path)
    }

    pub fn unique_mkdir(&mut self) -> Result<UniquePath, FatalError> {
        let (path, identifier) = self.random_path_in();
        fs::create_dir(&path)?;
        Ok(UniquePath {
            path,
            identifier,
        })
    }

    /// FIXME: async.
    pub fn store_to_file(&mut self, from: &mut dyn io::BufRead) -> Result<PathBuf, io::Error> {
        let (path, _) = self.random_path_in();
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        io::copy(from, &mut file)?;
        Ok(path)
    }

    pub fn work_dir(&self) -> &Path {
        &self.tempdir
    }

    pub fn import(&mut self, path: PathBuf) {
        self.imported.push(path)
    }

    pub fn imported(&mut self) -> impl Iterator<Item=PathBuf> + '_ {
        self.imported.drain(..)
    }

    fn random_path_in(&mut self) -> (PathBuf, Identifier) {
        let mut id = [0u8; 16];
        self.trng.fill(&mut id);
        (self.path_of(id), id)
    }
}

impl SyncSink {
    pub fn as_sink(&self) -> Sink {
        Sink {
            tempdir: self.path.clone(),
            trng: rand::thread_rng(),
            imported: vec![],
        }
    }

    pub fn work_dir(&self) -> &Path {
        &self.path
    }
}

impl From<Sink> for SyncSink {
    fn from(sink: Sink) -> SyncSink {
        SyncSink { path: sink.tempdir }
    }
}
