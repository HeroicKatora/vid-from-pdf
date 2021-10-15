fn build_library() {
    let tempdir = std::env::var("OUT_DIR")
        .unwrap();
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .unwrap();
    let messages = escargot::CargoBuild::default()
        .current_release()
        .current_target()
        .manifest_path(manifest)
        .package("mkv-slide-show")
        .bin("mkv-slide-show")
        .target_dir(tempdir)
        .exec()
        .unwrap();

    let messages: Vec<_> = messages
        .filter_map(Result::ok)
        .collect();

    let messages = messages
        .iter()
        .filter_map(|msg| {
            match msg.decode() {
                Ok(escargot::format::Message::CompilerArtifact(art)) => Some(art),
                _ => None,
            }
        });

}

fn main() {
    auditable_build::collect_dependency_list();
    build_library();
}
