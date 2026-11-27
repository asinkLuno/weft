//! `weft` 命令行工具：检查（`weft check`）、格式化（`weft fmt`）、编译（`weft build`）与语言服务。

mod build_cmd;
mod files;
mod fmt_cmd;
mod render;

use std::{collections::HashSet, path::PathBuf, process::ExitCode};

use clap::{Parser, Subcommand};
use weft_core::{
    compiler::{CheckOptions, check_document},
    errors::Severity,
    lint::LINT_RULES,
};

use crate::build_cmd::{BuildArgs, run_build};
use crate::files::{InputSource, collect_weft_paths, format_rel_path, is_stdin_arg};
use crate::fmt_cmd::{FmtArgs, run_fmt};
use crate::render::{
    JsonDiagnostic, OutputFormat, render_file_diagnostics, should_use_color,
};

#[derive(Parser)]
#[command(
    name = "weft",
    version,
    about = "Fast syntax/semantic linter and formatter for the Weft story language"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check `.weft` files for syntax, semantic, script, and lint diagnostics
    Check {
        /// Files or directories to check (defaults to `.`; pass `-` for stdin)
        paths: Vec<PathBuf>,
        /// Diagnostic output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Full)]
        output_format: OutputFormat,
        /// Skip loading and executing external `.rn` calendar/calculable-prop scripts
        #[arg(long)]
        syntax_only: bool,
        /// Calendar / script locale (`zh` or `en`; defaults to system locale)
        #[arg(long)]
        lang: Option<String>,
        /// Ignore specific diagnostic rules (comma-separated or repeatable)
        #[arg(long, value_delimiter = ',')]
        ignore: Vec<String>,
        /// Exit with non-zero status when warnings are present
        #[arg(long)]
        deny_warnings: bool,
        /// Disable ANSI colors in diagnostic output
        #[arg(long)]
        no_color: bool,
    },
    /// Format `.weft` files (idempotent, preserves comments)
    #[command(alias = "format")]
    Fmt {
        /// Files or directories to format (defaults to `.`; pass `-` for stdin)
        paths: Vec<PathBuf>,
        /// Check formatting without writing files (exits 1 if changes are needed)
        #[arg(long)]
        check: bool,
        /// Show unified diff of formatting changes without writing files
        #[arg(long)]
        diff: bool,
        /// Read input from stdin and print formatted output to stdout
        #[arg(long)]
        stdin: bool,
        /// Disable ANSI colors in syntax error output
        #[arg(long)]
        no_color: bool,
    },
    /// Compile a `.weft` file (with `.rn` scripts) to structured JSON
    #[command(alias = "compile")]
    Build {
        /// `.weft` file to compile (pass `-` for stdin)
        path: PathBuf,
        /// Write JSON output to a file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Calendar / script locale (`zh` or `en`; defaults to system locale)
        #[arg(long)]
        lang: Option<String>,
        /// Emit compact single-line JSON instead of pretty-printed JSON
        #[arg(long)]
        compact: bool,
        /// Disable ANSI colors in diagnostic output
        #[arg(long)]
        no_color: bool,
    },
    /// Start the Weft Language Server over stdio (LSP 3.17)
    Lsp {
        /// Default diagnostic / hover locale (`zh` or `en`; defaults to system locale)
        #[arg(long)]
        lang: Option<String>,
    },
    /// List available lint warning rules
    Rules,
}

fn default_lang() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            if val.to_ascii_lowercase().starts_with("zh") {
                return "zh".to_owned();
            }
            if !val.is_empty() {
                return "en".to_owned();
            }
        }
    }
    "en".to_owned()
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check {
            paths,
            output_format,
            syntax_only,
            lang,
            ignore,
            deny_warnings,
            no_color,
        } => run_check(
            paths,
            output_format,
            syntax_only,
            lang.unwrap_or_else(default_lang),
            ignore,
            deny_warnings,
            no_color,
        ),
        Command::Fmt {
            paths,
            check,
            diff,
            stdin,
            no_color,
        } => run_fmt(FmtArgs {
            paths,
            check,
            diff,
            stdin,
            no_color,
        }),
        Command::Build {
            path,
            output,
            lang,
            compact,
            no_color,
        } => run_build(BuildArgs {
            path,
            output,
            lang: lang.unwrap_or_else(default_lang),
            compact,
            no_color,
        }),
        Command::Lsp { lang } => {
            if let Err(err) = weft_ls::stdio::run_stdio(lang.unwrap_or_else(default_lang)) {
                eprintln!("error: weft lsp exited with failure: {err}");
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            }
        }
        Command::Rules => {
            for rule in LINT_RULES {
                println!("{rule}");
            }
            ExitCode::SUCCESS
        }
    }
}

fn run_check(
    paths: Vec<PathBuf>,
    output_format: OutputFormat,
    syntax_only: bool,
    lang: String,
    ignore: Vec<String>,
    deny_warnings: bool,
    no_color: bool,
) -> ExitCode {
    let sources = match load_sources(&paths) {
        Ok(sources) => sources,
        Err(err) => {
            eprintln!("error: {err}");
            return ExitCode::from(2);
        }
    };

    if sources.is_empty() {
        if output_format == OutputFormat::Json {
            println!("[]");
        } else {
            eprintln!("No .weft files found.");
        }
        return ExitCode::SUCCESS;
    }

    let ignored: HashSet<String> = ignore.into_iter().collect();
    let options = CheckOptions {
        run_scripts: !syntax_only,
        lang,
    };
    let color = should_use_color(no_color);

    let mut total_errors = 0usize;
    let mut total_warnings = 0usize;
    let mut json_diags = Vec::new();

    for source in &sources {
        let out = check_document(&source.content, &source.base_dir, &options);
        let filtered: Vec<_> = out
            .diagnostics
            .into_iter()
            .filter(|d| !ignored.contains(d.rule))
            .collect();

        for d in &filtered {
            match d.severity {
                Severity::Error => total_errors += 1,
                Severity::Warning => total_warnings += 1,
            }
            if output_format == OutputFormat::Json {
                json_diags.push(JsonDiagnostic::from_diagnostic(
                    &source.display_name,
                    &source.content,
                    d,
                ));
            }
        }

        if !filtered.is_empty() {
            render_file_diagnostics(
                &source.display_name,
                &source.content,
                &filtered,
                output_format,
                color,
            );
        }
    }

    if output_format == OutputFormat::Json {
        match serde_json::to_string_pretty(&json_diags) {
            Ok(text) => println!("{text}"),
            Err(err) => {
                eprintln!("error: failed to serialize diagnostics: {err}");
                return ExitCode::from(2);
            }
        }
    } else if output_format != OutputFormat::Github {
        let total = total_errors + total_warnings;
        if total == 0 {
            eprintln!("All checks passed! ({} file(s) checked)", sources.len());
        } else {
            eprintln!(
                "Found {total} diagnostic(s) ({total_errors} error(s), {total_warnings} warning(s))"
            );
        }
    }

    if total_errors > 0 || (deny_warnings && total_warnings > 0) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn load_sources(paths: &[PathBuf]) -> Result<Vec<InputSource>, String> {
    if is_stdin_arg(paths) {
        let src = InputSource::from_stdin().map_err(|e| format!("failed to read stdin: {e}"))?;
        return Ok(vec![src]);
    }
    let files = collect_weft_paths(paths)?;
    let mut sources = Vec::with_capacity(files.len());
    for path in files {
        let mut src = InputSource::from_path(path.clone())
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        src.display_name = format_rel_path(&path);
        sources.push(src);
    }
    Ok(sources)
}
