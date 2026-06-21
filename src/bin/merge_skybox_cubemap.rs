//! Merge loose skybox face PNGs into `assets/environment/skyboxes/{set}/cubemap.png`.
//!
//! ```text
//! cargo run --bin merge_skybox_cubemap -- default
//! ```

use std::env;
use std::process::ExitCode;

use chasma::environment::{loose_faces_exist, merge_loose_faces, DEFAULT_SKYBOX_SET, SKYBOX_ASSET_ROOT};

fn main() -> ExitCode {
    let set_name = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_SKYBOX_SET.to_string());

    if !loose_faces_exist(&set_name) {
        eprintln!(
            "No complete face set in assets/{SKYBOX_ASSET_ROOT}/{set_name}/\n\
             Expected: right.png, left.png, top.png, bottom.png, front.png, back.png"
        );
        return ExitCode::from(1);
    }

    match merge_loose_faces(&set_name) {
        Ok(path) => {
            println!("Wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("merge failed: {err}");
            ExitCode::from(1)
        }
    }
}
