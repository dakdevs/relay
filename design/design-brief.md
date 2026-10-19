# Relay — design brief

Status: **final designs ready for approval.** GPUI application UI won't be implemented until the user approves the boards on the "Final Designs" page.

- Paper page: <https://app.paper.design/file/01M58Z151ZVWZ4VVK921R3Q5J9/p-2-0> (file "Relay — Tray & Settings (design review)", page **Final Designs**)
- Superseded drafts: page **Drafts / Archive** (`p-1-0`). Every board there is prefixed "Archived ·". They are kept for history and aren't part of the approval.
- Screenshots: `design/screenshots/final/` (2× exports, reviewed at native resolution). Older exports are in `design/screenshots/archive/`.
- Generators: `design/paper-src/` (Executor `execute` programs; design tooling only). Logo sources are in `design/assets/logo-sources.md`.
- Implementation target: a Rust Cargo workspace with GPUI, and no JS/TS runtime, web UI shell or sidecar. The design generators aren't part of the product.

Every number, label and identity on the boards is a **design sample**. Today is fixed at Sun Oct 18, 09:41 local time.

## Final boards

| #   | Board                            | Contents                                                                                                                                                                      | Screenshot                              |
| --- | -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- |
| 0   | Index                            | Inventory and style rules                                                                                                                                                     | `00-index.png`                          |
| 1   | Tray popover — collapsed         | Light (Week) and dark (Month); provider sections, then plan groups with one bar per window; accounts in use; pinned footer with Through Relay, Pause, Settings and Open Relay | `01-tray-popover-collapsed.png`         |
| 2   | Tray popover — expanded          | Account cards with masked identifier, last seen, every window and its reset, reset credits and state tags; focus as a filled tint; Linux SNI scene                            | `02-tray-popover-expanded.png`          |
| 3   | Main window — Quotas ledger      | Filters with counts, search, View/Sort, Refresh all; shared provider summary; grouped ledger; capability-gated Reset limits and Refresh quota                                 | `03-quotas-ledger.png`                  |
| 4   | Accounts & connect               | Provider → plan or credential → account; disconnected (Reconnect) and paused (Resume); connect sheet with provider picker and modes; Codex Existing-setup sheet               | `04-accounts-connect.png`               |
| 5   | Usage, Through Relay & routing   | Large Week/Month chart; per-account token table; routing, quota-data and endpoint settings; capabilities table                                                                | `05-usage-through-relay-routing.png`    |
| 6   | States & numbers                 | In use, healthy, low, exhausted, stale, unknown, disconnected, paused, group average; empty first-run and not-routing popovers                                                | `06-states-and-numbers.png`             |
| 7   | Native menu fallback             | macOS NSMenu and Linux dbusmenu with Open Relay as the default item                                                                                                           | `07-native-menu-fallback.png`           |
| 8   | Buttons, interaction & motion    | Rest/hover/pressed/keyboard-focus/disabled for primary, secondary and icon buttons in light and dark; row states; motion table; acceptance criteria; keyboard; screen reader  | `08-buttons-interaction-motion.png`     |
| 9   | Through Relay — sparkline states | Week, hover/focus, month, partial period, no prior, insufficient prior, missing vs zero, no traffic; definitions                                                              | `09-through-relay-sparkline-states.png` |

## Style rules (user corrections, all applied)

- No borders or strokes. Separation comes from tonal surfaces, spacing and alternating row tints. Radii: popover 16, sections 12–14, rows and cards 9–10, buttons 7, tags 5.
- The reference screenshot governs the **layout only**. Its warm charcoal palette and strokes aren't used.
- Official provider and product marks only; no letter tiles. The Anthropic symbol marks the provider and the Claude spark marks the Claude plan. The OpenAI symbol marks the provider and the ChatGPT logo marks the ChatGPT plan. No separate Codex mark was sourced. Provenance and hashes: `assets/logo-sources.md`.
- No predicted-account or "next" indicator anywhere. "In use" describes current state, and "Resets in …" labels time.
- Every account keeps a stable label (Personal, Work, Secondary…), a masked identifier, its plan and its state, grouped by provider and then by plan or credential.
- Open Relay stays pinned in the tray footer and also appears in the native menu. It opens the main window on Quotas.
- Buttons are tactile: they rest on a gentle gradient with a short shadow, press down about 1 px with an inset shade and no layout shift, and turn flat and quiet when disabled. Focus and selection use filled tints, never outlines.

## Data semantics

- **Group** = provider + plan + credential type. A **window** is one provider-reported quota kind (5-hour, weekly, a model 7-day limit, or API tokens per minute).
- Bars show quota **left**. A group average covers only accounts with fresh data (under 15 min) in one comparable group and window, and the count is always printed. Stale and unknown values are counted separately. Unknown shows as "—", never 0%. Providers, plans and windows are never averaged together.
- Pooled capacity is the sum of each account's percentage left, written "X% of N × 100%". Segments stay per account. Resets show the first reset plus the range.
- Reset credits and their expiry appear only when the provider reports them. **Reset limits** runs the provider's own action and never clears Relay's counters. **Refresh quota** uses a documented usage read, never a billed request. Each is disabled, with the reason, when unsupported.
- **Pooled** (opt-in; terms shown as unresolved): each new session goes to the account with the most headroom, and running sessions stay sticky. **Observed**: the T3 instance keeps its own login, and Relay records usage only. The mode is set per account.
- Usage tokens are summed, never averaged, and count only traffic through Relay. Defaults: quota-aware placement, sticky sessions, idle polling off.

## Through Relay sparkline definitions

- Measures input + output tokens of requests that passed through Relay, in daily buckets in the local time zone. It isn't quota and isn't an account total.
- **Week** is the calendar week, with the first weekday taken from the locale. It compares week start → now with last week start → the same elapsed time.
- **Month** is the calendar month by the same rule. When the prior month is shorter than today's day-of-month, the prior span ends on its last day and the label says so.
- The current bucket is partial and drawn as a hollow end dot. Null means no data and leaves a gap over a tinted band. Zero is a real zero and gets a baseline dot.
- The delta appears only when both spans are fully recorded and the prior total is above zero. Otherwise a muted label shows instead: Too early, No comparison, Prior incomplete or Incomplete.
- Usage records are aggregated asynchronously into bounded per-day totals. The UI reads a snapshot; nothing runs on the routing path or triggers network calls.

## Motion spec (native GPUI)

| Interaction             | Motion                                                                                                                      | Reduced motion          |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| Pointer-down            | Pressed on the same frame: sinks 1 px with inset shade. Release returns in 120 ms, ease-out (0.23, 1, 0.32, 1)              | Shade only, no movement |
| Keyboard Space / Return | Static pressed shade, then the action; no spatial movement                                                                  | Same                    |
| Hover                   | Tint in 120 ms ease-out                                                                                                     | Instant                 |
| Popover open            | 160 ms ease-out: opacity, 4 px drop and 98→100% scale from the tray anchor; fade only when the compositor places the window | 120 ms fade             |
| Popover close           | 120 ms reverse of open, starting from the current presentation                                                              | Fade                    |
| Expand / collapse       | Height and chevron together in 180 ms, reversible mid-way                                                                   | Instant                 |
| Week / Month            | Pill slides in 160 ms; sparkline crossfades in 120 ms                                                                       | Instant                 |
| Telemetry               | Updates in place; no counting, interpolation, staggers or chasing                                                           | Same                    |

Bounds: every animation is at most 300 ms, with no scale(0), ease-in, overshoot or bounce. Acceptance must be **verified later with native recordings**: 120 fps on macOS and 60 fps on Linux.

- Press latency is at most 1 frame.
- Open, close and expand each drop at most 1 frame, and p95 frame time stays within budget.
- Interrupting an animation reverses it from its current values; Esc closes within 120 ms.
- Reduced-motion settings are honoured.

Static boards can't demonstrate smoothness.

## Reference coverage checklist

Each datum or action in the user's reference screenshot, and where it appears.

| Reference item                                         | Where                                                          |
| ------------------------------------------------------ | -------------------------------------------------------------- |
| Provider filters with counts                           | Board 3 filter chips (All 8 · Anthropic 6 · OpenAI 2)          |
| Search by label, email or credential file              | Board 3                                                        |
| View and Sort selectors                                | Board 3                                                        |
| Refresh all                                            | Board 3 title bar (tactile button)                             |
| Pooled capacity "X% of N×100% left"                    | Board 3 summary; tray group bars show the average (boards 1–2) |
| Average plus fresh/stale counts                        | Board 3 summary; tray "avg · n fresh"; board 6 rule            |
| Per-account segments                                   | Board 3 summary segments                                       |
| First reset plus range                                 | Board 3 summary                                                |
| Collapsible model limit                                | Board 3 summary (Model limit · Fable 5 · 7-day, Hide)          |
| Per-account weekly / 5-hour / model, each with reset   | Board 3 ledger; tray cards (board 2)                           |
| Reset credits and expiry, only if reported             | Board 3 Reset credits column; tray Personal card (board 2)     |
| Reset limits (capability-gated, reason when disabled)  | Board 3 actions                                                |
| Refresh quota (capability-gated, reason when disabled) | Board 3 actions                                                |
| Plan per account                                       | Board 3 (Max 20×, Plus); board 4 plan column                   |
| Account state (in use, exhausted, stale)               | Boards 2, 3, 4, 6                                              |
| Grouping by provider and plan                          | Boards 1–5                                                     |

## Platform caveats (implementation, not product copy)

- macOS: a borderless GPUI window positioned from the status item frame. Coexistence of tray-icon and GPUI on the main thread is unverified.
- Linux: SNI via `ksni`. Left-click arrives as `Activate` and opens the popover; right-click opens the native menu (board 7). Hosts differ, and some send zero coordinates or no Activate.
  - Wayland gives clients no global positioning. Anchoring needs layer-shell (KDE, wlroots), and GPUI support for it is unverified.
  - GNOME needs the AppIndicator extension and places the window itself.
  - X11 uses the click point.
- The Linux build uses the system UI font and solid surfaces, and bundles no Apple fonts or SF Symbols.
- GPUI accessibility-tree support must be verified before implementation.
- T3 Codex integration requires `setupMode: existing` and is untested live. A Claude router key replaces the subscription login.

## Visual tokens

- Light: canvas `#EEF0EF`, popover `#F2F3F2`, surface `#FFFFFF`, inset `#F4F5F4`, track `#E6E9E7`, text `#1C1F1E`, muted `#646A67`, evergreen `#1F6B4F`, amber `#9A6614`, ember `#B4432F`.
- Dark: popover `#1B1D1C`, surface `#262927`, inset `#2E3230`, evergreen `#4DB389`, amber `#D9A140`, ember `#E26A52`.
- Type: SF Pro on macOS and the system UI font on Linux, 10–17 pt in the UI, with tabular numerals. The popover is 360 px wide.
