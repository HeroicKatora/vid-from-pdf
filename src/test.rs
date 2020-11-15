use std::{fs, io};
use crate::{app, project, resources};

const PDF: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/test-data/test.pdf");
const WAV0: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/test-data/espeak-0.wav");
const WAV1: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/test-data/espeak-1.wav");
const WAV2: &'static str = concat!(env!("CARGO_MANIFEST_DIR"), "/test-data/espeak-2.wav");

#[test]
fn assemble() {
    let cfg = resources::Configuration::from_env()
        .expect("Configured from environment");
    let resources = resources::Resources::force(&cfg)
        .expect("Found all resources");
    let app = app::App::new(resources);

    let pdf = io::Cursor::new(fs::read(PDF).expect("Pdf file"));
    let project = project::Project::new(&mut app.sink.as_sink(), &mut {pdf})
        .expect("Project created");

    {
        for entry in fs::read_dir(app.sink.work_dir()).unwrap() {
            println!("{:?}", entry);
        }
        for entry in fs::read_dir(project.dir.work_dir()).unwrap() {
            println!("{:?}", entry);
        }

        match project::Project::load(&app, project.project_id) {
            Ok(Some(_)) => {}
            Ok(None) => panic!("Unexpectedly didn't find the project in {}", project.dir.work_dir().display()),
            Err(err) => panic!("Unexpectedly didn't load the project {:?}", err),
        };
    }
}
