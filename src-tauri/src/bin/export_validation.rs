#[path = "../validation.rs"]
#[allow(dead_code)]
mod validation;

use std::{fs, path::Path};
use ts_rs::{Config, TS};

fn main() {
    let output = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/generated/validation-error.ts");
    fs::create_dir_all(output.parent().expect("generated directory"))
        .expect("create generated directory");
    let binding = validation::ValidationError::export_to_string(&Config::default())
        .expect("export validation error type");
    fs::write(output, binding).expect("write validation error type");
}
