//! Headless Relay proxy.
//!
//! ```text
//! relay-proxy [--home DIR] [--config FILE] [run|check|version|help]
//! ```

use relay_proxy::paths::Paths;
use relay_proxy::{secrets, store};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "relay-proxy — Relay's loopback proxy without the desktop app

Usage: relay-proxy [--home DIR] [--config FILE] [COMMAND]

Commands:
  run      Start the proxy (default). Stops on Ctrl-C or SIGTERM.
  check    Validate the config and report each account's credential status.
  version  Print the version.
  help     Print this help.

Options:
  --home DIR     Relay directory (default: $RELAY_HOME, else the per-user location)
  --config FILE  Config file (default: DIR/config.toml)";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut home: Option<PathBuf> = None;
    let mut config: Option<PathBuf> = None;
    let mut command = String::from("run");
    while let Some(a) = args.next() {
        match a.as_str() {
            "--home" => home = args.next().map(PathBuf::from),
            "--config" => config = args.next().map(PathBuf::from),
            "-h" | "--help" | "help" => command = "help".into(),
            "-V" | "--version" | "version" => command = "version".into(),
            "run" | "check" => command = a,
            other => {
                eprintln!("unknown argument: {other}\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match command.as_str() {
        "help" => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        "version" => {
            println!("relay-proxy {}", relay_core::DISPLAY_VERSION);
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    let mut paths = match home {
        Some(h) => Paths::in_dir(&h),
        None => match Paths::for_user() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("cannot find Relay directory: {e}");
                return ExitCode::FAILURE;
            }
        },
    };
    if let Some(c) = config {
        paths.config_file = c;
    }
    let cfg = match store::load(&paths.config_file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}: {e}", paths.config_file.display());
            return ExitCode::FAILURE;
        }
    };
    if command == "check" {
        println!("config: {} (valid)", paths.config_file.display());
        println!("listen: {}", cfg.server.listen);
        if cfg.accounts.is_empty() {
            println!("accounts: none configured");
        }
        let mut ok = true;
        for a in &cfg.accounts {
            let status = match &a.credential {
                None => "client's own login (observed)".to_string(),
                Some(r) => match secrets::resolve(r) {
                    Ok(_) => "credential available".into(),
                    Err(e) => {
                        ok = false;
                        format!("credential unavailable: {e}")
                    }
                },
            };
            println!("{}", account_line(a, &status));
        }
        return if ok {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    let handle = match relay_proxy::start(&paths, cfg) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("relay-proxy: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "relay-proxy {} listening on http://{}",
        relay_core::DISPLAY_VERSION,
        handle.addr()
    );
    println!("local key: {}", paths.local_key_file.display());
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("relay-proxy: {e}");
            return ExitCode::FAILURE;
        }
    };
    rt.block_on(async {
        #[cfg(unix)]
        {
            let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = async { match term.as_mut() { Some(t) => { t.recv().await; } None => std::future::pending().await } } => {}
            }
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
    });
    eprintln!("relay-proxy: draining (up to 10 s)");
    handle.shutdown();
    ExitCode::SUCCESS
}

/// One `check` line. Names the vendor; for vendors that share another's
/// wire, the wire too, so a Kimi account never reads as Anthropic.
fn account_line(a: &relay_core::config::AccountConfig, status: &str) -> String {
    let vendor = a.vendor();
    let who = if vendor.is_first_party() {
        vendor.label().to_string()
    } else {
        format!("{} · {} wire", vendor.label(), a.provider.label())
    };
    format!(
        "account {} [{who} · {}{}]: {status}",
        a.id,
        a.mode.label(),
        if a.paused { " · paused" } else { "" },
    )
}

#[cfg(test)]
mod tests {
    use super::account_line;
    use relay_core::config::AccountConfig;
    use relay_core::model::{AccountId, AccountMode, Provider, Vendor};

    #[test]
    fn check_lines_name_the_vendor_not_only_the_wire() {
        let mut a = AccountConfig {
            id: AccountId("k".into()),
            provider: Provider::Anthropic,
            vendor: None,
            device_id: None,
            label: "K".into(),
            mode: AccountMode::Pooled,
            plan: None,
            identifier: None,
            credential_kind: None,
            upstream: None,
            credential: None,
            paused: false,
            max_in_flight: 8,
            models: vec![],
        };
        // First-party output is unchanged.
        assert_eq!(account_line(&a, "ok"), "account k [Anthropic · Pooled]: ok");
        a.vendor = Some(Vendor::KimiCom);
        a.paused = true;
        assert_eq!(
            account_line(&a, "ok"),
            "account k [Kimi · Anthropic wire · Pooled · paused]: ok"
        );
    }
}
