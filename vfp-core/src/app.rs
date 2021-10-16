//! Thred-safe abstraction for a whole app.
//!
//! The goal is that it's easy to bind this to any web server implementation.
use std::sync::{Arc, atomic::AtomicU64, atomic::Ordering};
use tempfile::TempDir;

use crate::explode::ExplodePdf;
use crate::ffmpeg::Ffmpeg;
use crate::sink::SyncSink;
use crate::resources::Resources;

/// All the state of dependencies, gathered from the OS.
pub struct App {
    /// The `ffmpeg` library / binary.
    pub ffmpeg: Ffmpeg,
    pub magick: svg_to_image::MagickConvert,
    /// The temporary directory for our runtime state.
    pub tempdir: TempDir,
    /// The container to collect files produced by us.
    pub sink: SyncSink,
    /// The program used for 'exploding' PDFs, that is splitting them into images page-by-page.
    pub explode: Arc<dyn ExplodePdf>,
    /// Runtime limits.
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
            magick: res.magick,
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
