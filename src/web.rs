use std::{fmt, io, sync::Arc};

use serde::Serialize;
use tokio::runtime;
use rand::Rng;
use rust_embed::RustEmbed;

use tide::{Request, Server};
use tide::http::mime;
use tide::sessions::{MemoryStore, SessionMiddleware};

use crate::{FatalError, sink};
use crate::app::App;
use crate::project::Project;

pub fn serve(app: App) -> Result<(), FatalError> {
    let state = Web::new(app)?;
    let app = tide_app(state);

    let rt = runtime::Builder::new_current_thread().build()?;
    rt.block_on(app.listen("localhost:8051"))?;

    Ok(())
}

#[derive(Clone)]
pub struct Web {
    arc: Arc<Static>,
}

struct Static {
    app: App,
    index: String,
}

#[derive(RustEmbed)]
#[folder = "public/"]
struct Asset;

impl Web {
    pub fn new(app: App) -> Result<Self, FatalError> {
        let index = Asset::get("index.html")
            .ok_or_else(|| {
                FatalError::Io(io::Error::new(
                    io::ErrorKind::NotFound,
                    "The main page asset was not found.",
                ))
            })?
            .into_owned();
        let index = String::from_utf8(index)
            .map_err(|err| {
                FatalError::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    err
                ))
            })?;
        Ok(Web {
            arc: Arc::new(Static {
                app,
                index,
            }),
        })
    }

    const PROJECT_ID: &'static str = "project-id";
}

fn serialize_project(project: &Project) -> impl Serialize {
    #[derive(Serialize)]
    struct Pages {
        pages: Vec<Page>,
    }

    #[derive(Serialize)]
    struct Page {
        img_url: Option<String>,
        audio_url: Option<String>,
    }

    Pages {
        pages: project.meta.slides
            .iter()
            .map(|slide| {
                Page {
                    img_url: None,
                    audio_url: None,
                }
            })
            .collect()
    }
}

fn tide_app(state: Web) -> Server<Web> {
    let mut app = tide::with_state(state);

    // We don't intend to store sessions, just use them to identify a user.
    let mut rng = rand::thread_rng();
    let ephemeral: [u8; 32] = rng.gen();
    app.with(SessionMiddleware::new(
        MemoryStore::new(),
        &ephemeral[..]
    ));

    app.at("/").get(tide_index);
    app.at("/project/new").put(tide_create);
    app.at("/project/page/:num").put(tide_set_audio);

    app
}

async fn tide_index(mut request: Request<Web>)
    -> tide::Result<tide::Response>
{
    let _ = request.session_mut();
    let content = request.state().arc.index.clone();
    let response = tide::Response::builder(200)
        .content_type(mime::HTML)
        .body(content)
        .build();
    Ok(response)
}

async fn tide_create(mut request: Request<Web>)
    -> tide::Result<tide::Response>
{
    // TODO: constify.
    let mime_pdf: mime::Mime = "application/pdf".parse().unwrap();

    match request.content_type() {
        Some(mime) if mime.essence() == mime_pdf.essence() => {},
        _ => {
            return Err(tide::Error::new(415, Error::OnlyPdfAccepted));
        }
    }

    match request.project()? {
        None => {},
        Some(project) => {
            // TODO: delete.
            request.session_mut().remove(Web::PROJECT_ID);
        }
    }

    let mut body = request
        .body_bytes()
        .await
        .map(io::Cursor::new)?;

    let mut sink = request.as_sink();

    let mut project = Project::new(&mut sink, &mut body)?;
    project.explode(&request.state().arc.app)?;

    request
        .session_mut()
        .insert(Web::PROJECT_ID, &project.project_id)?;
    tide_project_state(&project)
}

async fn tide_set_audio(mut request: Request<Web>)
    -> tide::Result<tide::Response>
{
    let page = request
        .url()
        .path_segments()
        .ok_or_else(|| tide::Error::new(500, Error::InternalServerError))?
        .rev()
        .next()
        .ok_or_else(|| tide::Error::new(400, Error::NoSuchProject))?;

    let idx = match page.parse() {
        Ok(idx) => idx,
        Err(_) => return Err(tide::Error::new(404, Error::NoSuchProject)),
    };

    let mut body = request
        .body_bytes()
        .await
        .map(io::Cursor::new)?;

    let mut project = request.require_project()?;
    let mut source = sink::BufSource::from(&mut body);

    project.import_audio(idx, &mut source)?;
    project.store()?;

    Ok(tide_project_state(&project)?)
}

fn tide_project_state(project: &Project) -> tide::Result<tide::Response> {
    let body = tide::Body::from_json(&serialize_project(project))?;

    let response = tide::Response::builder(201)
        .body(body)
        .content_type(mime::JSON)
        .build();

    Ok(response)
}

#[derive(Debug)]
enum Error {
    InternalServerError,
    NoSuchProject,
    OnlyPdfAccepted,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InternalServerError => f.write_str("An internal server error occurred."),
            Error::NoSuchProject => f.write_str("This project has been deleted."),
            Error::OnlyPdfAccepted => f.write_str("Only pdf is accepted."),
        }
    }
}

impl std::error::Error for Error {}

impl From<FatalError> for tide::Error {
    fn from(err: FatalError) -> tide::Error {
        eprintln!("{:?}", err);
        tide::Error::new(500, Error::InternalServerError)
    }
}

trait TideAppProject {
    fn project(&self) -> Result<Option<Project>, FatalError>;
    fn require_project(&self) -> tide::Result<Project>;
    fn as_sink(&self) -> sink::Sink;
}

impl TideAppProject for Request<Web> {
    fn project(&self) -> Result<Option<Project>, FatalError> {
        match self.session().get(Web::PROJECT_ID) {
            None => return Ok(None),
            Some(identifier) => Project::load(&self.state().arc.app, identifier),
        }
    }
    fn require_project(&self) -> tide::Result<Project> {
        match self.project()? {
            None => Err(tide::Error::new(410, Error::NoSuchProject)),
            Some(project) => Ok(project),
        }
    }
    fn as_sink(&self) -> sink::Sink {
        self.state().arc.app.sink.as_sink()
    }
}