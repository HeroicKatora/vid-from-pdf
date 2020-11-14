//! Thred-safe abstraction for a whole app.
//!
//! The goal is that it's easy to bind this to any web server implementation.
use std::sync::{Arc, atomic::AtomicU64, atomic::Ordering};
use tempfile::TempDir;

use crate::explode::ExplodePdf;
use crate::ffmpeg::Ffmpeg;
use crate::sink::SyncSink;
use crate::resources::Resources;

pub struct App {
    pub ffmpeg: Ffmpeg,
    pub tempdir: TempDir,
    pub sink: SyncSink,
    pub explode: Arc<dyn ExplodePdf>,
    pub limits: Limits,
}

/// Application wide limits.
///
/// Atomics so we can adjust them while running. However, we rarely will do so it's just a
/// precautionary measure to ensure we are reminded when adding complex limits that can not be
/// modified through a shared, sync reference.
pub struct Limits {
    pub meta_size: AtomicU64,
}

impl App {
}

impl Limits {
    pub fn meta_size(&self) -> u64 {
        self.meta_size.load(Ordering::Relaxed)
    }
}

impl App {
    pub fn new(res: Resources) -> App {
        App {
            ffmpeg: res.ffmpeg,
            tempdir: res.tempdir,
            sink: res.dir_as_sink.into(),
            explode: res.explode.into(),
            limits: Limits::default(),
        }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            meta_size: AtomicU64::new(2_000_000),
        }
    }
}

const _: () = {
    fn only_if_send_and_sync<T: Send + Sync>() {}
    let _ = only_if_send_and_sync::<App>;
};
