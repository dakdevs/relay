//! "Open at login": a per-user LaunchAgent (macOS) or XDG autostart entry
//! (Linux). Written or removed only when the user flips the switch.

use std::path::{Path, PathBuf};

pub const LABEL: &str = "dev.dak.relay";

/// Where the login item lives for `home` (and `xdg_config` on Linux).
pub fn item_path(home: &Path, xdg_config: Option<&Path>) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/LaunchAgents")
            .join(format!("{LABEL}.plist"))
    } else {
        xdg_config
            .map_or_else(|| home.join(".config"), Path::to_path_buf)
            .join("autostart/relay.desktop")
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn item_contents(exe: &Path) -> String {
    let exe = exe.to_string_lossy();
    if cfg!(target_os = "macos") {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key><array><string>{}</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
"#,
            escape_xml(&exe)
        )
    } else {
        format!(
            "[Desktop Entry]\nType=Application\nName=Relay\nExec={}\nX-GNOME-Autostart-enabled=true\nNoDisplay=true\n",
            desktop_exec_arg(&exe)
        )
    }
}

/// Quotes one argument for a Desktop Entry `Exec` key (freedesktop Desktop
/// Entry spec, "The Exec key"): inside double quotes, `"`, `` ` ``, `$` and
/// `\` are backslash-escaped; then the general string escape doubles each
/// backslash; `%` becomes `%%` so it is not read as a field code.
pub fn desktop_exec_arg(arg: &str) -> String {
    let mut quoted = String::with_capacity(arg.len() + 2);
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    format!("\"{}\"", quoted.replace('\\', "\\\\").replace('%', "%%"))
}

pub fn is_enabled(path: &Path) -> bool {
    path.is_file()
}

pub fn set_enabled(path: &Path, exe: &Path, enabled: bool) -> std::io::Result<()> {
    if enabled {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, item_contents(exe))
    } else {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

/// The login item for the current user, if HOME is known.
pub fn current() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    Some(item_path(Path::new(&home), xdg.as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_exec_quoting_follows_the_spec() {
        // Plain path: just quoted.
        assert_eq!(
            desktop_exec_arg("/opt/relay/relay"),
            r#""/opt/relay/relay""#
        );
        // `"`, `` ` ``, `$` get a quoting backslash, which the string escape
        // then doubles; a literal backslash becomes four; `%` becomes `%%`.
        assert_eq!(desktop_exec_arg(r#"/a "b"/r"#), r#""/a \\"b\\"/r""#);
        assert_eq!(desktop_exec_arg("/a`b$c/r"), r#""/a\\`b\\$c/r""#);
        assert_eq!(desktop_exec_arg(r"/a\b/r"), r#""/a\\\\b/r""#);
        assert_eq!(desktop_exec_arg("/50%/r"), r#""/50%%/r""#);
    }

    #[test]
    fn toggling_writes_and_removes_only_the_item() {
        let dir = std::env::temp_dir().join(format!("relay-login-test-{}", std::process::id()));
        let path = item_path(&dir, Some(&dir.join("xdg")));
        assert!(!is_enabled(&path));
        set_enabled(
            &path,
            Path::new("/Applications/Relay & Co.app/Contents/MacOS/relay"),
            true,
        )
        .unwrap();
        assert!(is_enabled(&path));
        let text = std::fs::read_to_string(&path).unwrap();
        if cfg!(target_os = "macos") {
            assert!(path.ends_with("Library/LaunchAgents/dev.dak.relay.plist"));
            assert!(text.contains("Relay &amp; Co.app"));
            assert!(text.contains("<key>RunAtLoad</key><true/>"));
        } else {
            assert!(path.ends_with("xdg/autostart/relay.desktop"));
            assert!(text.contains(&format!(
                "Exec={}",
                desktop_exec_arg("/Applications/Relay & Co.app/Contents/MacOS/relay")
            )));
        }
        set_enabled(&path, Path::new("/x"), false).unwrap();
        assert!(!is_enabled(&path));
        // Removing twice is fine.
        set_enabled(&path, Path::new("/x"), false).unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }
}
