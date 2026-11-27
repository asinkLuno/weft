//! `weft build`：将 `.weft` 文档（含历法与可计算属性 `.rn` 脚本）编译为结构化 JSON。

use std::{fs, path::PathBuf, process::ExitCode};

use weft_core::compiler::{CheckOptions, check_document};

use crate::files::{InputSource, format_rel_path};
use crate::render::{OutputFormat, render_file_diagnostics, should_use_color};

pub struct BuildArgs {
    pub path: PathBuf,
    pub output: Option<PathBuf>,
    pub lang: String,
    pub compact: bool,
    pub no_color: bool,
}

pub fn run_build(args: BuildArgs) -> ExitCode {
    let source = if args.path.as_os_str() == "-" {
        match InputSource::from_stdin() {
            Ok(src) => src,
            Err(err) => {
                eprintln!("error: failed to read stdin: {err}");
                return ExitCode::from(2);
            }
        }
    } else {
        match InputSource::from_path(args.path.clone()) {
            Ok(mut src) => {
                src.display_name = format_rel_path(&args.path);
                src
            }
            Err(err) => {
                eprintln!("error: failed to read {}: {err}", args.path.display());
                return ExitCode::from(2);
            }
        }
    };

    let options = CheckOptions {
        run_scripts: true,
        lang: args.lang,
    };
    let out = check_document(&source.content, &source.base_dir, &options);

    let errors: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|d| d.is_error())
        .cloned()
        .collect();
    if !errors.is_empty() {
        let color = should_use_color(args.no_color);
        render_file_diagnostics(
            &source.display_name,
            &source.content,
            &errors,
            OutputFormat::Full,
            color,
        );
        return ExitCode::FAILURE;
    }

    let Some(doc) = out.doc else {
        eprintln!("error: compilation produced no output");
        return ExitCode::FAILURE;
    };

    let json = if args.compact {
        serde_json::to_string(&doc)
    } else {
        serde_json::to_string_pretty(&doc)
    };
    let json = match json {
        Ok(text) => text,
        Err(err) => {
            eprintln!("error: failed to serialize compiled document: {err}");
            return ExitCode::from(2);
        }
    };

    if let Some(output_path) = args.output {
        if let Err(err) = fs::write(&output_path, format!("{json}\n")) {
            eprintln!("error: failed to write {}: {err}", output_path.display());
            return ExitCode::from(2);
        }
    } else {
        println!("{json}");
    }

    ExitCode::SUCCESS
}
