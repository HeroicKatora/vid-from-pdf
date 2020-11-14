//! Thred-safe abstraction for a whole app.
//!
//! The goal is that it's easy to bind this to any web server implementation.
use std::sync::Arc;
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
}

impl App {
}

impl From<Resources> for App {
    fn from(res: Resources) -> App {
        App {
            ffmpeg: res.ffmpeg,
            tempdir: res.tempdir,
            sink: res.dir_as_sink.into(),
            explode: res.explode.into(),
        }
    }
}

const _: () = {
    fn only_if_send_and_sync<T: Send + Sync>() {}
    let _ = only_if_send_and_sync::<App>;
};
