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
use crate::sink::FileSource;

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
    select: Option<(FileSelect, SelectTarget)>,
    project: Option<Project>,
    status: Option<String>,
    outfile: Option<PathBuf>,
    slide_idx: usize,
}

struct FileSelect {
    path: PathBuf,
    idx: usize,
    // TODO: redundant. Use sprint-dir or walk-dir here.
    files: Vec<PathBuf>,
    state: widgets::ListState,
}

enum SelectTarget {
    AudioOf(usize),
    Project,
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
                if let Some((ref mut select, _)) = tui.select {
                    let max = select.files.len();
                    select.idx = (select.idx.min(max)).wrapping_sub(1);
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
            }) => {
                if let Some((ref mut select, _)) = tui.select {
                    let max = select.files.len();
                    let next = select.idx.wrapping_add(1);
                    select.idx = if next < max { next } else { usize::MAX };
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            }) => {
                match tui.select.take() {
                    Some((select, SelectTarget::Project)) => {
                        tui.select_project(app, select)?
                    }
                    Some((select, SelectTarget::AudioOf(idx))) => {
                        tui.select_slide_audio(select, idx)?;
                    }
                    None => {
                        tui.compute_video(app)?;
                    }
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::NONE,
            }) => {
                if let Some(ref project) = tui.project {
                    if tui.slide_idx < project.meta.slides.len() {
                        tui.select = Some((tui.start_select()?, SelectTarget::AudioOf(tui.slide_idx)));
                        tui.slide_idx += 1;
                    }
                } else {
                    if tui.select.is_none() {
                        tui.select = Some((tui.start_select()?, SelectTarget::Project));
                    }
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char('s'),
                modifiers: KeyModifiers::NONE,
            }) => {
                if let Some(ref outfile) = tui.outfile {
                    fs::copy(outfile, "/tmp/output.mp4")?;
                } else {
                    tui.status = Some("No output file has been generated yet.".into());
                }
            }
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

        if let Some(ref project) = self.project {
            let block = widgets::Block::default()
                .title(format!("Project with {} slides", project.meta.slides.len()))
                .borders(widgets::Borders::ALL);
            frame.render_widget(block, size);

            let mut inner = size.inner(&layout::Margin { horizontal: 1, vertical: 1 });
            for (idx, slide) in project.meta.slides.iter().enumerate() {
                let item_rect = layout::Rect { height: 2, ..inner };
                let par = widgets::Paragraph::new(format!(
                        "{}Video: {}\n\
                         {}Audio: {}",
                         " ",
                         match &slide.visual {
                             crate::project::Visual::Slide { src, .. } => src.display(),
                         },
                         if idx == self.slide_idx { "*" } else { " " },
                         match &slide.audio {
                             None => String::from("Not yet selected"),
                             Some(src) => src.display().to_string(),
                         }
                    ));
                frame.render_widget(par, item_rect);
                inner.y = inner.y.saturating_add(2);
                inner.height = inner.height.saturating_sub(2);
            }
        }

        if let Some((ref mut select, ref kind)) = self.select {
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
                .title(match *kind {
                    SelectTarget::Project => format!("Select a pdf: {}", select.path.display()),
                    SelectTarget::AudioOf(idx) => format!("Select audio for slide {}", idx),
                })
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
            files: FileSelect::read_dir(Path::new("."))?,
            state: widgets::ListState::default(),
        })
    }

    fn select_project(&mut self, app: &App, select: FileSelect) -> Result<(), FatalError> {
        let selected_file = match self.resolve_file_selection(select, SelectTarget::Project) {
            Some(file) => file,
            None => return Ok(()),
        };

        let mut sink = app.sink.as_sink();
        let file = match fs::File::open(selected_file) {
            Err(io) => {
                self.status = Some(format!("Failed to open file: {:?}", io));
                return Ok(())
            },
            Ok(file) => file,
        };

        let mut file = io::BufReader::new(file);
        let mut project = Project::new(&mut sink, &mut file)?;
        project.explode(app)?;
        self.project = Some(project);

        Ok(())
    }

    fn select_slide_audio(&mut self, select: FileSelect, idx: usize)
        -> Result<(), FatalError>
    {
        let selected_file = match self.resolve_file_selection(select, SelectTarget::AudioOf(idx)) {
            Some(file) => file,
            None => return Ok(()),
        };

        let project = match self.project {
            Some(ref mut project) => project,
            None => {
                self.status = Some("Selecting an audio file without project does nothing. How did you end up here?".into());
                return Ok(())
            }
        };

        if project.meta.slides.len() <= idx {
            self.status = Some(format!("Slide index {} is out of range. How did you end up here?", idx));
            return Ok(())
        }

        let mut source = match FileSource::new_from_existing(selected_file) {
            Ok(source) => source,
            Err(err) => {
                self.status = Some(format!("Error opening selected audio file: {:?}", err));
                return Ok(());
            }
        };

        project.import_audio(idx, &mut source)?;
        self.status = Some(format!("Audio for slide {} was imported, moving to next slide", idx));
        Ok(())
    }

    fn compute_video(&mut self, app: &App) -> Result<(), FatalError> {
        let project = match self.project {
            Some(ref mut project) => project,
            None => {
                self.status = Some("Generating video file without project does nothing. How did you end up here?".into());
                return Ok(())
            }
        };

        if let Some(first) = project.meta.slides.iter().position(|slide| slide.audio.is_none()) {
            self.status = Some(format!("Slide {} does not have any audio selected, jumping to it.", first));
            self.slide_idx = first;
            return Ok(());
        }

        let assembly = project.assemble(app)?;
        let mut outsink = &mut app.sink.as_sink();
        assembly.finalize(&app.ffmpeg, &mut outsink)?;

        let outfile = match outsink.imported().next() {
            Some(pathbuf) => pathbuf,
            None => {
                self.status = Some("Error: Apparently no output was produced".into());
                return Ok(())
            }
        };

        self.outfile = Some(outfile);
        Ok(())
    }

    fn resolve_file_selection(&mut self, mut select: FileSelect, kind: SelectTarget)
        -> Option<PathBuf>
    {
        let selected_file = if let Some(select) = select.take_selected() {
            select
        } else {
            self.status = Some("no file selected".into());
            return None;
        };

        match fs::metadata(&selected_file) {
            Err(io) => {
                self.status = Some(format!("Failed to inspect file: {:?}", io));
                return None;
            }
            Ok(meta) if meta.is_dir() => {
                if let Err(err) = select.pivot(selected_file) {
                    self.status = Some(format!(
                        "Can't switch to directory, failed to canonicalize: {}", err
                    ));
                }
                self.select = Some((select, kind));
                return None;
            }
            Ok(meta) if !meta.is_file() => {
                self.status = Some("Neither a file nor a directory".into());
                return None;
            }
            Ok(_) => Some(selected_file),
        }
    }
}

impl FileSelect {
   fn take_selected(&mut self) -> Option<PathBuf> {
       match self.files.get_mut(self.idx) {
            None => None,
            Some(item) => Some(mem::take(item)),
       }
   }

   fn pivot(&mut self, folder: PathBuf) -> Result<(), io::Error> {
        self.files = Self::read_dir(Path::new(&folder))?;
        if let Ok(canonical) = folder.canonicalize() {
            self.path = canonical;
        }
        Ok(())
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
}
