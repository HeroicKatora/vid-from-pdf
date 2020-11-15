use std::{fs, io, path};
use crate::{app, project, resources, sink};

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
    let mut project = project::Project::new(&mut app.sink.as_sink(), &mut {pdf})
        .expect("Project created");

    { // Test we can load it..
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

    project.explode(&app)
        .expect("Exploding pdf failed");
    assert_eq!(project.meta.slides.len(), 3);

    for (idx, &wav) in [WAV0, WAV1, WAV2].iter().enumerate() {
        let path = path::Path::new(wav).to_owned();
        let mut source = sink::FileSource::new_from_existing(path)
            .expect("Input file to exist");
        project.import_audio(idx, &mut source)
            .expect("Audio file has been imported");
    }

    let assembly = project.assemble(&app)
        .expect("Had everything ready");
    let mut outsink = &mut app.sink.as_sink();
    assembly.finalize(&app.ffmpeg, &mut outsink)
        .expect("Assembly works");

    let output = outsink.imported().next()
        .expect("One output file");
}
