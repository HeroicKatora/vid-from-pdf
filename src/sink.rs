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

/// A path and its unique identifier.
pub struct UniqueFile {
    pub file: fs::File,
    /// Fully qualified directory for the project.
    pub path: PathBuf,
    /// Identifier for that project.
    pub identifier: Identifier,
}

pub type Identifier = [u8; 16];

pub trait Source {
    fn as_buf_read(&mut self) -> &mut dyn io::BufRead;
    fn as_path(&self) -> Option<&Path>;
}

pub struct FileSource {
    file: io::BufReader<fs::File>,
    path: PathBuf,
}

pub struct BufSource<'lt> {
    from: &'lt mut dyn io::BufRead,
}

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

    pub fn unique_path(&mut self) -> Result<UniquePath, FatalError> {
        let (path, identifier) = self.random_path_in();

        if path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Random path was an existing file"
            ).into());
        }

        Ok(UniquePath {
            path,
            identifier,
        })
    }

    pub fn unique_mkdir(&mut self) -> Result<UniquePath, FatalError> {
        let (path, identifier) = self.random_path_in();
        fs::create_dir(&path)?;
        Ok(UniquePath {
            path,
            identifier,
        })
    }

    /// Create a new file.
    /// This method will always set `options.create_new(true)`.
    pub fn unique_file(&mut self, options: &mut fs::OpenOptions) -> Result<UniqueFile, FatalError> {
        let (path, identifier) = self.random_path_in();
        let file = options
            .create_new(true)
            .open(&path)?;
        Ok(UniqueFile {
            file,
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

impl FileSource {
    /// Create by opening a file assumed to exist.
    pub fn new_from_existing(path: PathBuf) -> Result<Self, io::Error> {
        let file = fs::File::open(&path)?;
        Ok(FileSource {
            file: io::BufReader::new(file),
            path,
        })
    }

    /// Besides the trait, we're sure to have a path here.
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

impl Source for FileSource {
    fn as_buf_read(&mut self) -> &mut dyn io::BufRead {
        &mut self.file
    }

    fn as_path(&self) -> Option<&Path> {
        Some(&self.path)
    }
}

impl<'lt, T: io::BufRead> From<&'lt mut T> for BufSource<'lt> {
    fn from(buf: &'lt mut T) -> Self {
        BufSource { from: buf }
    }
}

impl Source for BufSource<'_> {
    fn as_buf_read(&mut self) -> &mut dyn io::BufRead {
        self.from
    }

    fn as_path(&self) -> Option<&Path> {
        None
    }
}
