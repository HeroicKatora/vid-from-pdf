use std::io;
use tokio::runtime;
use tokio::stream::StreamExt;
use crossterm::{
    ErrorKind,
    terminal::{disable_raw_mode, enable_raw_mode},
    event::{Event, EventStream, KeyCode, KeyEvent},
};
use tui::Terminal;
use tui::backend::CrosstermBackend;

use crate::FatalError;
use crate::app::App;

pub fn tui(app: App) -> Result<(), FatalError> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    let rt = runtime::Builder::new_current_thread().build()?;
    rt.block_on(drive_tui(terminal, &app))?;

    Ok(())
}

async fn drive_tui(
    mut term: Terminal<impl tui::backend::Backend>,
    app: &App,
)
    -> Result<(), FatalError>
{
    struct DisableRawMode;
    impl DisableRawMode {
        pub fn new() -> Result<Self, FatalError> {
            enable_raw_mode().map_err(convert_err)?;
            Ok(DisableRawMode)
        }
    }
    impl Drop for DisableRawMode {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
        }
    }

    let _canary = DisableRawMode::new();
    let mut events = EventStream::new();
    loop {
        let next = match events.next().await {
            Some(event) => event,
            None => break,
        };

        // TODO: maybe some deliberation on some error types?
        match next.map_err(convert_err)? {
            Event::Key(KeyEvent { 
                code: KeyCode::Char('q'),
                ..
            }) => break,
            _ => {}
        }

        term.draw(draw_tui)?;
    }
    Ok(())
}

fn convert_err(ct: ErrorKind) -> FatalError {
    match ct {
        ErrorKind::IoError(io) => io.into(),
        other => io::Error::new(io::ErrorKind::Other, other).into(),
    }
}

fn draw_tui(frame: &mut tui::Frame<'_, impl tui::backend::Backend>) {
}
