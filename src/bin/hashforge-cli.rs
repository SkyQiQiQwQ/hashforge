use hashforge::{hash_bytes, hash_file, validate_hex, Algorithm, ALGORITHMS, DEFAULT_ALGORITHM};
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Command {
    Hash,
    Text,
    Compare,
    Verify,
}

struct Options {
    command: Command,
    positional: Vec<OsString>,
    text: Option<String>,
    expected: Option<String>,
    algorithms: Vec<Algorithm>,
    all: bool,
}

fn parse_options(mut args: impl Iterator<Item = OsString>) -> Result<Options, String> {
    let command = args
        .next()
        .and_then(|arg| arg.into_string().ok())
        .ok_or_else(usage)?;
    let command = match command.as_str() {
        "hash" => Command::Hash,
        "text" => Command::Text,
        "compare" => Command::Compare,
        "verify" => Command::Verify,
        "--help" | "-h" | "help" => return Err(usage()),
        "--version" | "-V" => return Err(format!("hashforge-cli {}", env!("CARGO_PKG_VERSION"))),
        _ => return Err(format!("unknown command '{command}'\n\n{}", usage())),
    };

    let mut positional = Vec::new();
    let mut text = None;
    let mut expected = None;
    let mut algorithms = Vec::new();
    let mut all = false;
    let mut positional_only = false;
    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        if positional_only {
            positional.push(argument);
            continue;
        }
        let Some(option) = argument.to_str() else {
            positional.push(argument);
            continue;
        };
        match option {
            "--help" | "-h" => return Err(usage()),
            "--version" | "-V" => {
                return Err(format!("hashforge-cli {}", env!("CARGO_PKG_VERSION")))
            }
            "--" => positional_only = true,
            "--all" => all = true,
            "--algorithm" | "-a" => {
                let value = args
                    .next()
                    .and_then(|arg| arg.into_string().ok())
                    .ok_or_else(|| "--algorithm requires a value".to_owned())?;
                for name in value.split(',') {
                    let algorithm = Algorithm::parse(name.trim()).ok_or_else(|| {
                        format!("unsupported algorithm '{name}'\n{}", algorithm_list())
                    })?;
                    if !algorithms.contains(&algorithm) {
                        algorithms.push(algorithm);
                    }
                }
            }
            "--text" => {
                if text.is_some() {
                    return Err("--text may be specified only once".to_owned());
                }
                text = Some(
                    args.next()
                        .and_then(|arg| arg.into_string().ok())
                        .ok_or_else(|| "--text requires a UTF-8 value".to_owned())?,
                );
            }
            "--hash" => {
                if expected.is_some() {
                    return Err("--hash may be specified only once".to_owned());
                }
                expected = Some(
                    args.next()
                        .and_then(|arg| arg.into_string().ok())
                        .ok_or_else(|| "--hash requires a value".to_owned())?,
                );
            }
            _ if option.starts_with('-') && option != "-" => {
                return Err(format!("unknown option '{option}'\n\n{}", usage()));
            }
            _ => positional.push(argument),
        }
    }

    if all && !algorithms.is_empty() {
        return Err("use either --all or --algorithm, not both".to_owned());
    }
    if all && command != Command::Hash {
        return Err("--all is available only with the hash command".to_owned());
    }
    if algorithms.len() > 1 && command != Command::Hash {
        return Err("select exactly one algorithm for text, compare, and verify".to_owned());
    }
    if command == Command::Text && text.is_some() {
        return Err(
            "text takes its input as a positional argument; --text is for compare".to_owned(),
        );
    }
    if command != Command::Compare && text.is_some() {
        return Err("--text is available only with compare".to_owned());
    }
    if command != Command::Verify && expected.is_some() {
        return Err("--hash is available only with verify".to_owned());
    }

    Ok(Options {
        command,
        positional,
        text,
        expected,
        algorithms,
        all,
    })
}

fn run(options: Options) -> Result<ExitCode, String> {
    let algorithms = if options.all {
        ALGORITHMS.to_vec()
    } else if options.algorithms.is_empty() {
        vec![DEFAULT_ALGORITHM]
    } else {
        options.algorithms.clone()
    };

    match options.command {
        Command::Hash => {
            require_positional_count(&options.positional, 1, "hash")?;
            let path = PathBuf::from(&options.positional[0]);
            for (algorithm, value) in hash_file(&path, &algorithms)? {
                println!("{}  {}", value, algorithm.name());
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Text => {
            require_positional_count(&options.positional, 1, "text")?;
            let text = options.positional[0]
                .to_str()
                .ok_or_else(|| "text input must be valid UTF-8".to_owned())?;
            let (algorithm, value) = hash_bytes(text.as_bytes(), &algorithms)[0].clone();
            println!("{}  {}", value, algorithm.name());
            Ok(ExitCode::SUCCESS)
        }
        Command::Compare => {
            require_positional_count(&options.positional, 1, "compare")?;
            let text = options
                .text
                .as_deref()
                .ok_or_else(|| "compare requires --text <ORIGINAL_TEXT>".to_owned())?;
            let algorithm = algorithms[0];
            let original = hash_bytes(text.as_bytes(), &[algorithm])[0].1.clone();
            let path = PathBuf::from(&options.positional[0]);
            let current = hash_file(&path, &[algorithm])?.remove(0).1;
            println!("text: {}", original);
            println!("file: {}", current);
            if original == current {
                println!("MATCH");
                Ok(ExitCode::SUCCESS)
            } else {
                println!("MISMATCH");
                Ok(ExitCode::from(1))
            }
        }
        Command::Verify => {
            require_positional_count(&options.positional, 1, "verify")?;
            let expected = options
                .expected
                .as_deref()
                .ok_or_else(|| "verify requires --hash <EXPECTED_HASH>".to_owned())?;
            let algorithm = algorithms[0];
            validate_hex(expected, algorithm)?;
            let path = PathBuf::from(&options.positional[0]);
            let actual = hash_file(&path, &[algorithm])?.remove(0).1;
            println!("{}  {}", actual, algorithm.name());
            if expected.eq_ignore_ascii_case(&actual) {
                println!("OK");
                Ok(ExitCode::SUCCESS)
            } else {
                println!("MISMATCH");
                Ok(ExitCode::from(1))
            }
        }
    }
}

fn require_positional_count(
    arguments: &[OsString],
    expected: usize,
    command: &str,
) -> Result<(), String> {
    if arguments.len() == expected {
        Ok(())
    } else {
        Err(format!(
            "{command} requires exactly {expected} positional argument(s)\n\n{}",
            usage()
        ))
    }
}

fn algorithm_list() -> String {
    format!(
        "Supported algorithms: {}",
        ALGORITHMS
            .iter()
            .map(|algorithm| algorithm.name())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn usage() -> String {
    format!(
        "Usage:\n  hashforge-cli hash <FILE|-> [--algorithm sha256,blake3 | --all]\n  hashforge-cli text <TEXT> [--algorithm sha256]\n  hashforge-cli compare <FILE> --text <ORIGINAL_TEXT> [--algorithm sha256]\n  hashforge-cli verify <FILE> --hash <EXPECTED_HEX> [--algorithm sha256]\n\n{}\n\nUse '-' as FILE to read from standard input.",
        algorithm_list()
    )
}

fn main() -> ExitCode {
    match parse_options(env::args_os().skip(1)).and_then(run) {
        Ok(code) => code,
        Err(error) => {
            if error == usage() || error.starts_with("hashforge-cli ") {
                println!("{error}");
                ExitCode::SUCCESS
            } else {
                eprintln!("error: {error}");
                ExitCode::from(2)
            }
        }
    }
}
