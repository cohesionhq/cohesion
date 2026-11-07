//! Compresses each file of the web UI's build into `OUT_DIR/web/` and writes
//! `OUT_DIR/web_files.rs`, the `(path, raw deflate)` table the daemon hands
//! clients (`WebBundleRequest`). With no build the table is empty and the
//! request fails, so the workspace still builds without Node.

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use flate2::{Compression, write::DeflateEncoder};

fn main() {
    println!("cargo:rerun-if-changed=../web/build");
    let build = Path::new("../web/build");
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let compressed = out.join("web");
    let _ = fs::remove_dir_all(&compressed);
    fs::create_dir_all(&compressed).expect("OUT_DIR is writable");

    let mut paths = Vec::new();
    if build.join("index.html").exists() {
        collect(build, build, &mut paths);
        paths.sort();
    } else {
        println!(
            "cargo:warning=web/build is missing, so cohesiond won't serve the web UI; run npm run build in web/"
        );
    }

    let mut table = String::from("&[\n");
    for (index, path) in paths.iter().enumerate() {
        let source = fs::read(build.join(path)).expect("the build's files are readable");
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
        encoder
            .write_all(&source)
            .expect("writing to a Vec cannot fail");
        let target = compressed.join(format!("{index}.deflate"));
        fs::write(
            &target,
            encoder.finish().expect("writing to a Vec cannot fail"),
        )
        .expect("OUT_DIR is writable");
        table.push_str(&format!("    ({path:?}, include_bytes!({target:?})),\n"));
    }
    table.push(']');
    fs::write(out.join("web_files.rs"), table).expect("OUT_DIR is writable");
}

/// Every file under `dir`, relative to `root` with `/` separators, but
/// `.gitkeep`, which only keeps the directory in git.
fn collect(root: &Path, dir: &Path, paths: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("the build's directories are readable") {
        let path = entry.expect("the build's directories are readable").path();
        if path.is_dir() {
            collect(root, &path, paths);
        } else if path.file_name().is_some_and(|name| name != ".gitkeep") {
            let relative = path.strip_prefix(root).expect("collected under root");
            let parts: Vec<_> = relative.iter().map(|part| part.to_string_lossy()).collect();
            paths.push(parts.join("/"));
        }
    }
}
