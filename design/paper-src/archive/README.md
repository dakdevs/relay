# Archived Paper generators

Superseded design tooling, kept for history only. Don't run these files: they recreate outdated boards with letter tiles, outlines and the removed predicted-account indicator.

- `boards-01-03.js`: native-menu-first boards 01–03.
- `parts.js`: early fragment helpers.
- `boards-02-03.js`, `boards-04-06.js`, `boards.js`: earlier tray, settings, states and interaction boards.

Current generators, all Paper design tooling and not application code:

- `../kit.js`: shared tokens, official marks, tactile buttons, sparkline and tray parts. Prepend it to a batch.
- `../batch-b.js`: tray collapsed, tray expanded and Linux, and sparkline states (final boards 1, 2, 9).
- `../board-09-quotas.js`: the Quotas ledger (final board 3).
- `../batch-c.js`: accounts and connect, usage and routing, states, native menu, and buttons and motion (final boards 4–8).

Relay itself is a Rust Cargo workspace using GPUI.
