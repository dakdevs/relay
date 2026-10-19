//! Command-line flags, handled before any window, tray, proxy or file access.

pub const HELP: &str = "\
Relay — local router for your own Claude and Codex accounts.

Usage: relay [--version | --help]

With no arguments Relay starts its local proxy and adds a menu bar (macOS)
or StatusNotifierItem (Linux) icon. Set RELAY_HOME to use another data
directory.

Options:
  -V, --version  Print the version and exit
  -h, --help     Print this help and exit
";

#[derive(Debug, PartialEq, Eq)]
pub enum Cli {
    Run,
    /// Print this text to stdout and exit 0.
    Print(String),
    /// Print this text to stderr and exit 2.
    Error(String),
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Cli {
    let args: Vec<String> = args.into_iter().skip(1).collect();
    match args.as_slice() {
        [] => Cli::Run,
        [a] if a == "--version" || a == "-V" => {
            Cli::Print(format!("relay {}\n", crate::DISPLAY_VERSION))
        }
        [a] if a == "--help" || a == "-h" => Cli::Print(HELP.to_string()),
        // macOS may pass a process serial number when launched from Finder.
        [a] if a.starts_with("-psn_") => Cli::Run,
        _ => Cli::Error(format!(
            "relay: unrecognised arguments: {}\n\n{HELP}",
            args.join(" ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Cli {
        parse(
            std::iter::once("relay")
                .chain(args.iter().copied())
                .map(String::from),
        )
    }

    #[test]
    fn version_is_relay_space_display_version() {
        let Cli::Print(out) = run(&["--version"]) else {
            panic!()
        };
        // Exactly `relay YYYYMMDD.N`.
        let v = out.strip_prefix("relay ").unwrap().trim_end();
        let (date, n) = v.split_once('.').unwrap();
        assert_eq!(date.len(), 8);
        assert!(date.chars().all(|c| c.is_ascii_digit()));
        assert!(n.parse::<u32>().is_ok());
        assert_eq!(run(&["-V"]), run(&["--version"]));
    }

    #[test]
    fn help_and_errors() {
        assert!(matches!(run(&["--help"]), Cli::Print(h) if h.contains("Usage: relay")));
        assert_eq!(run(&[]), Cli::Run);
        assert_eq!(run(&["-psn_0_12345"]), Cli::Run);
        assert!(matches!(run(&["--bogus"]), Cli::Error(_)));
    }
}
