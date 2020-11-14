use std::{fs, io, mem, path::Path, path::PathBuf};
use tokio::runtime;
use tokio::stream::StreamExt;
use crossterm::{
    ErrorKind,
    terminal::{disable_raw_mode, enable_raw_mode},
    event::{Event, EventStream, KeyCode, KeyEvent, KeyModifiers},
};
use tui::{Terminal, layout, widgets};
use tui::backend::CrosstermBackend;

use crate::FatalError;
use crate::app::App;
use crate::project::Project;

pub fn tui(app: App) -> Result<(), FatalError> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;

    let rt = runtime::Builder::new_current_thread().build()?;
    rt.block_on(drive_tui(terminal, &app))?;

    Ok(())
}

#[derive(Default)]
struct Tui {
    select: Option<FileSelect>,
    project: Option<Project>,
    status: Option<String>,
}

struct FileSelect {
    path: PathBuf,
    idx: usize,
    // TODO: redundant. Use sprint-dir or walk-dir here.
    files: Vec<PathBuf>,
    state: widgets::ListState,
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
    let mut tui = Tui::default();
    tui.status = Some("Press `n` to create a new project.".into());

    term.clear()?;
    term.draw(|frame| tui.draw(frame))?;

    loop {
        let next = match events.next().await {
            // TODO: maybe some deliberation on some error types?
            Some(event) => event.map_err(convert_err)?,
            None => break,
        };

        match next {
            Event::Key(KeyEvent {
                code: KeyCode::Char('q'),
                ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
            }) => break,
            Event::Key(KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
            }) => {
                if let Some(ref mut select) = tui.select {
                    let max = select.files.len();
                    select.idx = (select.idx.min(max)).wrapping_sub(1);
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
            }) => {
                if let Some(ref mut select) = tui.select {
                    let max = select.files.len();
                    let next = select.idx.wrapping_add(1);
                    select.idx = if next < max { next } else { usize::MAX };
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            }) => {
                if let Some(select) = tui.select.take() {
                    tui.select_project(app, select)?;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
            }) => {
                if tui.select.is_none() {
                    tui.select = Some(tui.start_select()?);
                }
            },
            _ => {}
        }

        term.draw(|frame| tui.draw(frame))?;
    }

    Ok(())
}

fn convert_err(ct: ErrorKind) -> FatalError {
    match ct {
        ErrorKind::IoError(io) => io.into(),
        other => io::Error::new(io::ErrorKind::Other, other).into(),
    }
}

impl Tui {
    fn draw(&mut self, frame: &mut tui::Frame<'_, impl tui::backend::Backend>) {
        let size = frame.size();
        frame.render_widget(widgets::Clear, size);

        if let Some(ref mut select) = self.select {
            let block_rect = size.inner(&layout::Margin { horizontal: 5, vertical: 5 });
            let rect = block_rect.inner(&layout::Margin { horizontal: 1, vertical: 1 });

            let items = select.files.len();
            let first = if select.idx < items {
                select.idx
                    .min(items)
                    .saturating_sub(5)
            } else {
                0
            };

            let last = items
                .min(first + usize::from(rect.height));

            let list = select.files[first..last]
                .iter()
                .map(|os| os.to_str().unwrap_or("???? Unreadable file name"))
                .map(|item| widgets::ListItem::new(item))
                .collect::<Vec<_>>();

            select.state.select(if (first..last).contains(&select.idx) {
                Some(select.idx - first)
            } else {
                None
            });

            let block = widgets::Block::default()
                .title(format!("Select a pdf: {}", select.path.display()))
                .borders(widgets::Borders::ALL);
            frame.render_widget(block, block_rect);
            let list = widgets::List::new(list).highlight_symbol("*");
            frame.render_widget(widgets::Clear, rect);
            frame.render_stateful_widget(list, rect, &mut select.state);
        }

        if let Some(ref status) = self.status {
            let rect = layout::Rect {
                x: 0,
                y: size.height.saturating_sub(1),
                height: 1,
                width: size.width,
            };

            frame.render_widget(widgets::Paragraph::new(status.as_str()), rect);
        }
    }

    fn start_select(&self) -> Result<FileSelect, io::Error> {
        Ok(FileSelect {
            path: Path::new(".").to_owned(),
            idx: usize::MAX,
            files: Self::read_dir(Path::new("."))?,
            state: widgets::ListState::default(),
        })
    }

    fn read_dir(path: &Path) -> Result<Vec<PathBuf>, io::Error> {
        let mut entries = fs::read_dir(path)?
            .map(|r| r.map(|entry| entry.path()))
            // TODO: potentially collecting for multiple seconds..
            .collect::<Result<Vec<PathBuf>, _>>()?;
        entries.push(path.join(".."));
        entries.sort();
        Ok(entries)
    }

    fn select_project(&mut self, app: &App, mut select: FileSelect) -> Result<(), FatalError> {
        let selected_file = match select.files.get_mut(select.idx) {
            None => {
                self.status = Some("no file selected".into());
                return Ok(())
            }
            Some(item) => mem::take(item),
        };

        match fs::metadata(&selected_file) {
            Err(io) => {
                self.status = Some(format!("Failed to inspect file: {:?}", io));
                return Ok(());
            }
            Ok(meta) if meta.is_dir() => {
                select.files = Self::read_dir(Path::new(&selected_file))?;
                select.path = selected_file;
                self.select = Some(select);
                return Ok(());
            }
            Ok(meta) if !meta.is_file() => {
                self.status = Some("Neither a file nor a directory".into());
                return Ok(());
            }
            Ok(_) => {},
        }

        let mut sink = app.sink.as_sink();
        let file = match fs::File::open(selected_file) {
            Err(io) => {
                self.status = Some(format!("Failed to open file: {:?}", io));
                return Ok(())
            },
            Ok(file) => file,
        };

        let mut file = io::BufReader::new(file);
        let project = Project::new(&mut sink, &mut file)?;
        self.project = Some(project);

        Ok(())
    }
}
