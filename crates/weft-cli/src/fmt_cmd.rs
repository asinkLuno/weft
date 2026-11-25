//! `weft fmt` 子命令：原地格式化、`--check`、`--diff` 与 stdin 模式。

use std::{fs, io::Write, path::PathBuf, process::ExitCode};

use similar::TextDiff;
use weft_core::format::format_weft;

use crate::files::{InputSource, collect_weft_paths, format_rel_path, is_stdin_arg};
use crate::render::{OutputFormat, render_file_diagnostics, should_use_color};

pub struct FmtArgs {
    pub paths: Vec<PathBuf>,
    pub check: bool,
    pub diff: bool,
    pub stdin: bool,
    pub no_color: bool,
}

pub fn run_fmt(args: FmtArgs) -> ExitCode {
    let color = should_use_color(args.no_color);

    if args.stdin || is_stdin_arg(&args.paths) {
        return run_fmt_stdin(args.check, args.diff, color);
    }

    let paths = match collect_weft_paths(&args.paths) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    if paths.is_empty() {
        eprintln!("No .weft files found.");
        return ExitCode::SUCCESS;
    }

    let mut changed = 0usize;
    let mut unchanged = 0usize;
    let mut syntax_errors = 0usize;

    for path in paths {
        let display = format_rel_path(&path);
        let src = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("error: failed to read {display}: {err}");
                return ExitCode::from(2);
            }
        };

        let formatted = match format_weft(&src) {
            Ok(f) => f,
            Err(diags) => {
                syntax_errors += diags.len();
                render_file_diagnostics(&display, &src, &diags, OutputFormat::Full, color);
                continue;
            }
        };

        if formatted == src {
            unchanged += 1;
            continue;
        }

        changed += 1;
        if args.diff {
            print_diff(&display, &src, &formatted);
        } else if args.check {
            eprintln!("Would reformat: {display}");
        } else if let Err(err) = fs::write(&path, &formatted) {
            eprintln!("error: failed to write {display}: {err}");
            return ExitCode::from(2);
        }
    }

    if syntax_errors > 0 {
        eprintln!(
            "Found {syntax_errors} syntax error(s); files with syntax errors were not formatted."
        );
        return ExitCode::FAILURE;
    }

    if args.check || args.diff {
        if changed > 0 {
            eprintln!("{changed} file(s) would be reformatted, {unchanged} file(s) left unchanged");
            return ExitCode::FAILURE;
        }
        eprintln!("{unchanged} file(s) already formatted");
        return ExitCode::SUCCESS;
    }

    eprintln!("{changed} file(s) reformatted, {unchanged} file(s) left unchanged");
    ExitCode::SUCCESS
}

fn run_fmt_stdin(check: bool, diff: bool, color: bool) -> ExitCode {
    let source = match InputSource::from_stdin() {
        Ok(s) => s,
        Err(err) => {
            eprintln!("error: failed to read stdin: {err}");
            return ExitCode::from(2);
        }
    };

    let formatted = match format_weft(&source.content) {
        Ok(f) => f,
        Err(diags) => {
            render_file_diagnostics(
                &source.display_name,
                &source.content,
                &diags,
                OutputFormat::Full,
                color,
            );
            return ExitCode::FAILURE;
        }
    };

    if diff {
        if formatted != source.content {
            print_diff(&source.display_name, &source.content, &formatted);
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }

    if check {
        return if formatted == source.content {
            ExitCode::SUCCESS
        } else {
            eprintln!("Would reformat: {}", source.display_name);
            ExitCode::FAILURE
        };
    }

    if let Err(err) = std::io::stdout().write_all(formatted.as_bytes()) {
        eprintln!("error: failed to write stdout: {err}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

fn print_diff(file: &str, old: &str, new: &str) {
    let diff = TextDiff::from_lines(old, new);
    print!(
        "{}",
        diff.unified_diff()
            .context_radius(3)
            .header(&format!("a/{file}"), &format!("b/{file}"))
    );
}
