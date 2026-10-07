//! Repository checks that use Rust syntax and Cargo target ownership.

mod paths;
mod rust_architecture;
mod rust_source;

use std::path::Path;
use std::process::ExitCode;

/// Reports check failures through the process status so Task and CI can enforce them.
fn main() -> ExitCode {
    let Some(workspace) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() else {
        eprintln!("xtask must be a direct workspace child");
        return ExitCode::FAILURE;
    };
    match run_with_arguments(std::env::args().skip(/*n*/ 1), workspace) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

/// Accepts CLI inputs explicitly so command errors can be tested without changing the checkout.
fn run_with_arguments(
    mut arguments: impl Iterator<Item = String>,
    workspace: &Path,
) -> Result<(), String> {
    let command = arguments
        .next()
        .ok_or_else(|| "usage: cargo xtask <check-rust-size|report-rust-size>".to_string())?;
    if let Some(unexpected) = arguments.next() {
        return Err(format!("unexpected argument `{unexpected}`"));
    }
    let operation = match command.as_str() {
        "check-rust-size" => rust_architecture::check_rust_architecture,
        "report-rust-size" => rust_architecture::report_rust_architecture,
        _ => return Err(format!("unknown xtask command `{command}`")),
    };
    operation(workspace).map_err(|error| format!("Rust architecture check failed: {error}"))
}

#[cfg(test)]
mod tests {
    use super::run_with_arguments;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    /// Invalid commands fail before invoking repository tools or changing files.
    #[test]
    fn rejects_missing_unknown_and_extra_arguments() {
        for (arguments, expected) in [
            (
                vec![],
                "usage: cargo xtask <check-rust-size|report-rust-size>",
            ),
            (vec!["unknown"], "unknown xtask command `unknown`"),
            (
                vec!["check-rust-size", "extra"],
                "unexpected argument `extra`",
            ),
            (
                vec!["report-rust-size", "extra"],
                "unexpected argument `extra`",
            ),
        ] {
            assert_eq!(
                run_with_arguments(arguments.into_iter().map(String::from), Path::new("unused")),
                Err(expected.to_string())
            );
        }
    }
}
