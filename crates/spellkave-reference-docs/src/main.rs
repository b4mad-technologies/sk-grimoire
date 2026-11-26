//! `--write` renders the reference pages; `--check` fails when a page differs.

use std::process::ExitCode;

fn main() -> ExitCode {
    let root = spellkave_reference_docs::repository_root();
    match std::env::args().nth(1).as_deref() {
        Some("--write") => match spellkave_reference_docs::write(&root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("cannot write the reference pages: {error}");
                ExitCode::FAILURE
            }
        },
        Some("--check") => {
            let drifted = spellkave_reference_docs::drifted(&root);
            for path in &drifted {
                eprintln!("{path} is out of date");
            }
            if drifted.is_empty() {
                ExitCode::SUCCESS
            } else {
                eprintln!("run: cargo run --locked -p spellkave-reference-docs -- --write");
                ExitCode::FAILURE
            }
        }
        _ => {
            eprintln!("usage: spellkave-reference-docs --write | --check");
            ExitCode::from(2)
        }
    }
}
