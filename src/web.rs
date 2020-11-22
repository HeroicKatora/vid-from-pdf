use std::{fmt, io, path, sync::Arc};

use serde::Serialize;
use tokio::runtime;
use rand::Rng;
use rust_embed::RustEmbed;

use tide::{Request, Server};
use tide::http::mime;
use tide::sessions::{MemoryStore, SessionMiddleware};

use crate::{FatalError, sink};
use crate::app::App;
use crate::project::{Project, Visual};

pub fn serve(app: App) -> Result<(), FatalError> {
    let state = Web::new(app)?;
    let app = tide_app(state);

    let rt = runtime::Builder::new_current_thread().build()?;

    let addr = "localhost:8051";
    eprintln!("Serving web server on `{}`", addr);
    rt.block_on(app.listen(addr))?;

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
        identifier: String,
        pages: Vec<Page>,
        output: Option<String>,
    }

    #[derive(Serialize)]
    struct Page {
        img_url: Option<String>,
        audio_url: Option<String>,
    }

    fn project_asset_url(path: &path::Path) -> String {
        // TODO: review. Or turn into static invariant.
        let name = path.file_name().unwrap();
        let name = std::path::Path::new(name);
        format!("/project/asset/{}", name.display())
    }

    fn slide_to_page(slide: &crate::project::Slide) -> Page {
        Page {
            img_url: match slide.visual {
                Visual::Slide { ref src, .. } => {
                    Some(project_asset_url(src))
                }
            },
            audio_url: match slide.audio {
                None => None,
                Some(ref src) => Some(project_asset_url(src)),
            },
        }
    }

    Pages {
        identifier: base64::encode_config(&project.project_id, base64::URL_SAFE),
        pages: project.meta.slides
            .iter()
            .map(slide_to_page)
            .collect(),
        output: match project.meta.output {
            None => None,
            Some(ref path) => Some(project_asset_url(path)),
        }
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
    // FIXME: restore the session state to that id.
    app.at("/project/edit/:id").get(tide_index);

    app.at("/project/new").put(tide_create);
    app.at("/project/get").get(tide_introspect);
    app.at("/project/asset/:id").get(tide_project_asset);
    app.at("/project/render").post(tide_render);

    app.at("/project/page/:num").put(tide_set_audio);
    app.at("/static/*").get(tide_static);

    app
}

async fn tide_index(mut request: Request<Web>)
    -> tide::Result<tide::Response>
{
    let _ = request.session_mut();
    #[cfg(not(debug_assertions))]
    let content = request.state().arc.index.clone();
    #[cfg(debug_assertions)]
    let content = {
        // Mark as used..
        let _ = request.state().arc.index;
        Asset::get("index.html").unwrap().into_owned() 
    };
    let response = tide::Response::builder(200)
        .content_type(mime::HTML)
        .body(content)
        .build();
    Ok(response)
}

async fn tide_introspect(request: Request<Web>)
    -> tide::Result<tide::Response>
{
    if let Some(project) = request.project()? {
        tide_project_state(&project)
    } else {
        Ok(tide::Response::builder(404).build())
    }
}

async fn tide_project_asset(request: Request<Web>)
    -> tide::Result<tide::Response>
{
    let path = {
        let project = match request.project()? {
            Some(project) => project,
            None => return Ok(tide::Response::builder(404).build()),
        };

        let path = request.url().path();
        let relative = path
            .strip_prefix("/project/asset/")
            .ok_or_else(|| tide::Error::new(400, Error::AssetNotFound))?;

        project.dir.work_dir().join(relative)
    };

    let body = tide::Body::from_file(path).await?;
    let response = tide::Response::builder(200)
        .body(body)
        .build();
    Ok(response)
}

async fn tide_render(request: Request<Web>)
    -> tide::Result<tide::Response>
{
    let mut project = match request.project()? {
        Some(project) => project,
        None => return Ok(tide::Response::builder(404).build()),
    };

    project.assemble(&request.state().arc.app)?;
    project.store()?;

    tide_project_state(&project)
}

async fn tide_static(request: Request<Web>)
    -> tide::Result<tide::Response>
{
    let path = request.url().path();
    let relative = path
        .strip_prefix("/static/")
        .ok_or_else(|| tide::Error::new(400, Error::AssetNotFound))?;
    let cow = Asset::get(relative)
        .ok_or_else(|| tide::Error::new(400, Error::AssetNotFound))?;
    let content = cow.into_owned();

    let extension = std::path::Path::new(relative)
        .extension()
        // We have an asset without file extension. Great.
        .ok_or_else(|| tide::Error::new(500, Error::InternalServerError))?
        .to_str()
        .ok_or_else(|| tide::Error::new(500, Error::InternalServerError))?;
    // Or one that isn't valid.. Good job.
    let mime = mime::Mime::from_extension(extension)
        .ok_or_else(|| tide::Error::new(500, Error::InternalServerError))?;

    let response = tide::Response::builder(200)
        .content_type(mime)
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
            let sink = request.state().arc.app.sink.as_sink();
            request.session_mut().remove(Web::PROJECT_ID);

            let path = sink.path_of(project.project_id);
            drop(project);
            let _ = std::fs::remove_dir_all(path);
        }
    }

    let mut body = request
        .body_bytes()
        .await
        .map(io::Cursor::new)?;

    let mut sink = request.as_sink();

    let mut project = Project::new(&mut sink, &mut body)?;
    project.explode(&request.state().arc.app)?;
    project.store()?;

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
    AssetNotFound,
    InternalServerError,
    NoSuchProject,
    OnlyPdfAccepted,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::AssetNotFound => f.write_str("No such asset."),
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
