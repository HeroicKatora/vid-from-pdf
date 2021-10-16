use std::path::PathBuf;
use escargot::format::{Message, WorkspaceMember};

fn path_from_local_workspace(member: WorkspaceMember) -> String {
    // Yeah, we do this by string parsing.
    let to_parse = format!("{:?}", member);
    let (_, path) = to_parse.split_once("file://")
        .unwrap_or_else(|| {
            panic!("expect to find a path spec in workspace member {:?}", member)
        });
    let path = path.split_once(")").map_or(path, |tup| tup.0);
    path.to_owned()
}

fn build_binary(bin: &str) -> PathBuf {
    let tempdir = std::env::var("OUT_DIR")
        .unwrap();
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap();
    let messages = escargot::CargoBuild::default()
        .current_release()
        .current_target()
        .manifest_path(manifest + "/../Cargo.toml")
        .package(bin)
        .target_dir(tempdir)
        .exec()
        .unwrap();

    //  Ensures there is no error finding messages.
    let messages: Vec<_> = messages
        .filter_map(|msg| match msg {
            Ok(msg) => Some(msg),
            Err(msg) => panic!("{:?}", msg),
        })
        .collect();

    //  Get the artifact output message.
    let mut messages = messages
        .iter()
        .filter_map(|msg| {
            match msg.decode() {
                Ok(Message::CompilerArtifact(art)) => {
                    let executable = art.executable?;
                    let path = path_from_local_workspace(art.package_id);
                    Some((executable, path))
                },
                Ok(other) => { eprintln!("{:?}", other); None },
                _other => None,
            }
        });

    let (executable, path) = messages.next()
        .expect("to have built a binary target");

    println!("cargo:rerun-if-changed={}", path);
    executable.into_owned()
}

fn main() {
    auditable_build::collect_dependency_list();
    let mkv_slide_show = build_binary("mkv-slide-show");
    println!("cargo:rustc-env=VFP_MKV_SLIDE_SHOW={}", mkv_slide_show.display());
    let mupdf_explode = build_binary("mupdf-explode");
    println!("cargo:rustc-env=VFP_MUPDF_EXPLODE={}", mupdf_explode.display());
}
