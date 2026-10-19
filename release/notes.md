Relay @VERSION@ for macOS (Apple Silicon, Intel) and Linux (x86_64, aarch64).

## Install

With the GitHub CLI logged in to an account that can read this repository:

```sh
gh release download v@VERSION@ --repo dakdevs/relay --pattern install.sh --dir /tmp/relay-@VERSION@
bash /tmp/relay-@VERSION@/install.sh
```

This release's `install.sh` carries the SHA-256 checksums of the archives below and installs only version @VERSION@. It writes to your home directory and never uses `sudo`.

| Archive                                | Contents                                                  |
| -------------------------------------- | --------------------------------------------------------- |
| `relay-@VERSION@-macos-aarch64.tar.gz` | `Relay.app` for Apple Silicon, macOS 12 or later          |
| `relay-@VERSION@-macos-x86_64.tar.gz`  | `Relay.app` for Intel Macs, macOS 12 or later             |
| `relay-@VERSION@-linux-x86_64.tar.gz`  | `bin/relay`, desktop entry and icons; glibc 2.35 or later |
| `relay-@VERSION@-linux-aarch64.tar.gz` | Same for 64-bit Arm                                       |
| `SHA256SUMS`                           | Checksums of every file in this release                   |

Each archive includes `THIRD-PARTY-LICENSES.md`. A few dependencies publish no license file anywhere; their entries say so and include the standard text of their declared license.

## Platform notes

- **macOS builds are ad-hoc signed, not signed with a Developer ID and not notarized.** The installer does not quarantine the app. If you download an archive with a browser instead, macOS blocks the first launch: open System Settings → Privacy & Security and choose Open Anyway.
- **Linux** needs a Vulkan driver, and a StatusNotifierItem host for the tray icon. GNOME needs the AppIndicator extension; without a host Relay opens its main window instead. Runtime requirements on Debian/Ubuntu: `libdbus-1-3 libfontconfig1 libfreetype6 libvulkan1 libwayland-client0 libx11-6 libx11-xcb1 libxcb1 libxkbcommon-x11-0 libxkbcommon0 libzstd1`.
