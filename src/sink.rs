/// Some owned tempdir or memory controller.
/// Basically, make it somewhat unlikely we clobber anything that other programs want to write or
/// control memory where we can enforce maximum usage. Give us a convenient interface around
/// requesting that particular paths may be kept freed for us. Provide a collector for collecting
/// output as files or in memory and transparently paged?
use std::{fs, io, path::Path, path::PathBuf};
use rand::{rngs::ThreadRng, Rng as _};
use tempfile::TempDir;

/// See module description.
///
/// TODO: prefix for non-colliding output bunch.
/// TODO: suffix control for store_to_file.
pub struct Sink {
    tempdir: TempDir,
    trng: ThreadRng,
}

impl Sink {
    pub fn store_to_file(&mut self, from: &mut dyn io::BufRead) -> Result<PathBuf, io::Error> {
        let path = self.random_path_in();
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)?;
        io::copy(from, &mut file)?;
        Ok(path)
    }

    pub fn work_dir(&self) -> &Path {
        self.tempdir.path()
    }

    fn random_path_in(&mut self) -> PathBuf {
        const SRC: &'static str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
        assert_eq!(SRC.len(), 64);

        let mut id = [0u8; 16];
        self.trng.fill(&mut id);

        let mut path = String::new();
        for &b in &id {
            let ch = SRC.chars().nth(usize::from(b & 63)).unwrap();
            path.push(ch);
        }

        let base = self.tempdir.path().to_owned();
        base.join(&path)
    }
}
