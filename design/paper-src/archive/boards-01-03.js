// Executor `execute` program that regenerates Relay boards 01–03 in Paper.
// Source of truth for the review boards; values are design examples only.
const fileId = "01M58Z151ZVWZ4VVK921R3Q5J9";
const F = `font-family:var(--font-ui);font-variant-numeric:tabular-nums;`;
const G = "#1C1F1E",
  L = "#6B716E",
  FR = "#ECEEED";
const col = {
  evergreen: "#1F6B4F",
  amber: "#B7791F",
  ember: "#B4432F",
  lichen: "#6B716E",
};
const level = (p) =>
  p == null ? null : p < 15 ? "ember" : p < 40 ? "amber" : "evergreen";
const idOf = (res, name) => {
  const s = JSON.stringify(res);
  if (!name) return s.match(/\\"id\\": \\"([0-9A-Z]+-[0-9]+)\\"/)[1];
  const re = new RegExp(
    `\\\\"id\\\\": \\\\"([0-9A-Z]+-[0-9]+)\\\\",\\\\n\\s*\\\\"name\\\\": \\\\"${name}\\\\"`,
  );
  return s.match(re)[1];
};

// ---------- shared pieces ----------
const chev = (o) =>
  `<svg width="10" height="10" viewBox="0 0 10 10" style="flex-shrink:0;transform:rotate(${o ? 90 : 0}deg)"><path d="M3.5 2 L7 5 L3.5 8" fill="none" stroke="#6B716E" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const hatch = `background-image:repeating-linear-gradient(135deg,#D5D9D7 0px,#D5D9D7 2px,transparent 2px,transparent 5px);background-color:${FR};`;
const tag = (t, c) =>
  `<div style="flex-shrink:0;padding:1px 6px;border-radius:4px;background:color-mix(in srgb, ${c} 12%, transparent);${F}font-size:9px;font-weight:600;letter-spacing:0.07em;color:${c}">${t}</div>`;
const win = ({
  label,
  pct,
  meta,
  stale = false,
  metaColor = L,
  indent = 18,
}) => {
  const lv = level(pct);
  const track =
    pct == null
      ? `<div style="flex:1;height:4px;border-radius:3px;${hatch}"></div>`
      : `<div style="flex:1;height:4px;border-radius:3px;background:${FR};overflow:hidden;display:flex"><div style="width:${pct}%;height:4px;border-radius:3px;background:${col[lv]};opacity:${stale ? 0.35 : 1}"></div></div>`;
  const vcol = pct == null || stale ? L : pct < 15 ? col.ember : G;
  return `<div style="display:flex;align-items:center;gap:10px;padding-left:${indent}px;${F}"><div style="width:58px;flex-shrink:0;font-size:10px;font-weight:500;letter-spacing:0.06em;color:${L};text-transform:uppercase;white-space:nowrap">${label}</div>${track}<div style="width:38px;flex-shrink:0;text-align:right;font-size:13px;font-weight:500;color:${vcol}">${pct == null ? "—" : pct + "%"}</div><div style="width:78px;flex-shrink:0;text-align:right;font-size:11px;color:${metaColor};white-space:nowrap">${meta}</div></div>`;
};
const head = ({ name, count, mode, open = false, note = "", noteColor = L }) =>
  `<div style="display:flex;align-items:center;gap:8px;${F}">${chev(open)}<div style="font-size:13px;font-weight:600;color:${G};white-space:nowrap">${name}</div><div style="font-size:11px;color:${L};white-space:nowrap">${count}</div>${mode === "pooled" ? tag("POOLED", col.evergreen) : tag("OBSERVED", col.lichen)}<div style="flex:1"></div><div style="font-size:11px;font-weight:500;color:${noteColor};white-space:nowrap">${note}</div></div>`;
const explain = (t) =>
  `<div style="padding-left:18px;${F}font-size:11px;line-height:16px;color:${L}">${t}</div>`;
const acct = ({ name, tagH = "", seen, seenColor = L, rows, note = "" }) =>
  `<div style="display:flex;flex-direction:column;gap:7px;padding:9px 10px;margin-left:18px;border-radius:8px;background:#F7F8F7"><div style="display:flex;align-items:center;gap:6px;${F}"><div style="font-size:12px;font-weight:600;color:${G}">${name}</div>${tagH}<div style="flex:1"></div><div style="font-size:11px;color:${seenColor};white-space:nowrap">${seen}</div></div>${rows.map((r) => win({ ...r, indent: 0 })).join("")}${note ? `<div style="${F}font-size:11px;line-height:16px;color:${L}">${note}</div>` : ""}</div>`;
const group = (h, rows) =>
  `<div style="display:flex;flex-direction:column;gap:8px;padding:12px 16px 14px;border-top:1px solid ${FR}">${h}${rows.join("")}</div>`;
const header = (legend) =>
  `<div layer-name="Header" style="display:flex;align-items:center;gap:8px;padding:14px 16px ${legend ? 4 : 12}px;${F}"><div style="font-size:15px;font-weight:600;color:${G};letter-spacing:-0.01em">Relay</div><div style="display:flex;align-items:center;gap:5px;padding:2px 8px 2px 6px;border-radius:10px;background:color-mix(in srgb, #1F6B4F 10%, transparent)"><div style="width:6px;height:6px;border-radius:3px;background:#1F6B4F"></div><div style="font-size:11px;font-weight:500;color:#1F6B4F">Running</div></div><div style="flex:1"></div><div style="font-size:11px;color:${L}">127.0.0.1:8317</div></div>${legend ? `<div style="padding:0 16px 12px;${F}font-size:11px;color:${L}">Bars show quota left · averages use fresh data (&lt; 15 min)</div>` : ""}`;
const nextRow = `<div layer-name="Next route" style="display:flex;align-items:center;gap:8px;padding:10px 16px;border-top:1px solid ${FR};background:#FAFBFA;${F}font-size:11px;color:${L}"><div style="flex-shrink:0;text-transform:uppercase;letter-spacing:0.06em;font-weight:500;font-size:10px">Next pooled</div><div style="flex-shrink:0;color:${G};font-size:12px;white-space:nowrap">Anthropic API · key-a</div><div style="flex:1"></div><div style="flex-shrink:0;white-space:nowrap">sticky · 3 sessions</div></div>`;
const menuItem = (label, key = "") =>
  `<div style="display:flex;align-items:center;height:26px;padding:0 10px;border-radius:6px;${F}font-size:13px;color:${G}"><div style="flex:1">${label}</div><div style="color:${L}">${key}</div></div>`;
const trayIcon = (c) =>
  `<svg width="16" height="16" viewBox="0 0 16 16"><circle cx="3.5" cy="8" r="1.8" fill="${c}"/><path d="M5.5 8 H8 M8 8 C10 8 10 4 12.5 4 M8 8 C10 8 10 12 12.5 12" fill="none" stroke="${c}" stroke-width="1.4" stroke-linecap="round"/><circle cx="13" cy="4" r="1.3" fill="${c}"/><circle cx="13" cy="12" r="1.3" fill="${c}"/></svg>`;
const menubar = (t) =>
  `<div style="display:flex;align-items:center;justify-content:flex-end;gap:14px;height:26px;padding:0 14px;background:rgba(255,255,255,0.55);${F}font-size:13px;color:${G}"><div style="display:flex;align-items:center;justify-content:center;width:28px;height:22px;border-radius:5px;background:rgba(0,0,0,0.12)">${trayIcon(G)}</div><div>${t}</div></div>`;
const popShell = (name, inner) =>
  `<div layer-name="${name}" style="display:flex;flex-direction:column;width:340px;margin:6px 16px 0 auto;background:#FFFFFF;border-radius:12px;box-shadow:0 14px 36px rgba(28,31,30,0.18),0 0 0 0.5px rgba(28,31,30,0.16);overflow:hidden">${inner}</div>`;
const title = (eyebrow, h, body) =>
  `<div layer-name="Board title" style="display:flex;flex-direction:column;gap:6px;${F}"><div style="font-size:11px;font-weight:500;letter-spacing:0.08em;text-transform:uppercase;color:#1F6B4F">${eyebrow}</div><div style="font-size:30px;font-weight:700;letter-spacing:-0.02em;color:${G}">${h}</div><div style="font-size:14px;line-height:21px;color:${L};max-width:780px">${body}</div></div>`;
const cap = (h, b) =>
  `<div style="display:flex;flex-direction:column;gap:4px;${F}"><div style="font-size:13px;font-weight:600;color:${G}">${h}</div><div style="font-size:12px;line-height:18px;color:${L}">${b}</div></div>`;
const boardStyles = {
  width: "1080px",
  height: "fit-content",
  background: "#FFFFFF",
  display: "flex",
  flexDirection: "column",
  gap: "32px",
  padding: "48px",
};

// ---------- group data (design examples) ----------
const gClaude = (open) =>
  group(head({ name: "Claude", count: "Max · 3", mode: "observed", open }), [
    win({ label: "5-hour", pct: 48, meta: "avg · 3 fresh" }),
    win({ label: "Weekly", pct: 71, meta: "avg · 3 fresh" }),
    ...(open
      ? [
          explain(
            "Each T3 instance keeps its own login. Relay records usage and quota; it does not switch accounts.",
          ),
          acct({
            name: "personal",
            seen: "seen 2m ago",
            rows: [
              { label: "5-hour", pct: 81, meta: "resets 2h 10m" },
              { label: "Weekly", pct: 88, meta: "resets 4d" },
            ],
          }),
          acct({
            name: "work",
            seen: "seen 4m ago",
            rows: [
              { label: "5-hour", pct: 64, meta: "resets 3h" },
              { label: "Weekly", pct: 70, meta: "resets 4d" },
            ],
          }),
          acct({
            name: "archive",
            tagH: tag("EXHAUSTED", col.ember),
            seen: "seen 1m ago",
            rows: [
              {
                label: "5-hour",
                pct: 0,
                meta: "resets 38m",
                metaColor: col.ember,
              },
              { label: "Weekly", pct: 54, meta: "resets 2d" },
            ],
            note: "This T3 instance is limited until the window resets.",
          }),
        ]
      : []),
  ]);
const gCodex = (open) =>
  group(
    head({
      name: "Codex",
      count: "Plus · 2",
      mode: "observed",
      open,
      note: "1 stale · 18m",
      noteColor: col.amber,
    }),
    [
      win({ label: "5-hour", pct: 22, meta: "avg · 1 fresh" }),
      win({ label: "Weekly", pct: 64, meta: "avg · 1 fresh" }),
      ...(open
        ? [
            acct({
              name: "dev",
              seen: "seen 3m ago",
              rows: [
                { label: "5-hour", pct: 22, meta: "resets 1h 05m" },
                { label: "Weekly", pct: 64, meta: "resets 6d" },
              ],
            }),
            acct({
              name: "team",
              tagH: tag("STALE", col.amber),
              seen: "seen 18m ago",
              seenColor: col.amber,
              rows: [
                { label: "5-hour", pct: 58, stale: true, meta: "not in avg" },
                { label: "Weekly", pct: null, meta: "not reported" },
              ],
              note: "Updates on this instance's next request.",
            }),
          ]
        : []),
    ],
  );
const gApi = (open) =>
  group(
    head({ name: "Anthropic API", count: "2 keys", mode: "pooled", open }),
    [
      win({ label: "Tokens/min", pct: 76, meta: "avg · 2 fresh" }),
      ...(open
        ? [
            explain(
              "Relay picks the key with the most headroom; a conversation stays on its key.",
            ),
            acct({
              name: "key-a",
              tagH: tag("NEXT", col.evergreen),
              seen: "seen 20s ago",
              rows: [{ label: "Tokens/min", pct: 84, meta: "resets 40s" }],
            }),
            acct({
              name: "key-b",
              seen: "seen 1m ago",
              rows: [{ label: "Tokens/min", pct: 68, meta: "resets 12s" }],
            }),
          ]
        : []),
    ],
  );

// ---------- board 02: collapsed panel ----------
const today = `<div style="display:flex;align-items:baseline;gap:6px;padding:12px 16px 4px;border-top:1px solid ${FR};${F}"><div style="font-size:11px;color:${L}">Today via Relay</div><div style="flex:1"></div><div style="font-size:13px;font-weight:500;color:${G}">1.24M</div><div style="font-size:11px;color:${L}">tokens · 312 req</div></div>`;
const menu = `<div style="display:flex;flex-direction:column;padding:6px 6px 8px">${menuItem("Pause routing")}${menuItem("Settings…", "⌘,")}${menuItem("Quit Relay", "⌘Q")}</div>`;
const pop02 = popShell(
  "Popover",
  header(false) +
    gClaude(false) +
    gCodex(false) +
    gApi(false) +
    nextRow +
    today +
    menu,
);

// ---------- board 01: native menus ----------
const mi = (
  label,
  { key = "", sub = false, dis = false, hi = false, color = null } = {},
) =>
  `<div style="display:flex;align-items:center;gap:6px;height:22px;padding:0 10px 0 6px;border-radius:5px;${hi ? "background:#2F6FDB;" : ""}${F}font-size:13px;color:${hi ? "#FFFFFF" : dis ? "#8E9390" : color || G}"><div style="width:8px;flex-shrink:0"></div><div style="flex:1;white-space:nowrap">${label}</div><div style="color:${hi ? "#FFFFFF" : "#9A9F9C"}">${key}${sub ? "›" : ""}</div></div>`;
const sep = `<div style="height:1px;background:rgba(0,0,0,0.09);margin:5px 10px"></div>`;
const nmenu = (w, items, extra = "") =>
  `<div style="display:flex;flex-direction:column;width:${w}px;flex-shrink:0;padding:5px;background:rgba(246,246,246,0.97);border-radius:9px;box-shadow:0 10px 30px rgba(0,0,0,0.20),0 0 0 0.5px rgba(0,0,0,0.18);${extra}">${items.join("")}</div>`;
const mainItems = [
  mi("Relay — Running on 127.0.0.1:8317", { dis: true }),
  sep,
  mi("Claude Max · 3 observed", { sub: true, hi: true }),
  mi("5h 48% left · wk 71% left · 3 fresh", { dis: true }),
  mi("Codex Plus · 2 observed", { sub: true }),
  mi("5h 22% left · wk 64% left · 1 stale", { dis: true }),
  mi("Anthropic API · 2 keys pooled", { sub: true }),
  mi("Tokens/min 76% left · 2 fresh", { dis: true }),
  sep,
  mi("Next pooled: Anthropic API · key-a", { dis: true }),
  mi("Today: 1.24M tokens · 312 requests", { dis: true }),
  sep,
  mi("Open Quota Panel…"),
  mi("Settings…", { key: "⌘," }),
  mi("Pause Routing"),
  sep,
  mi("Quit Relay", { key: "⌘Q" }),
];
const subItems = [
  mi("personal — 5h 81% · wk 88%"),
  mi("work — 5h 64% · wk 70%"),
  mi("archive — exhausted · resets 38m", { color: col.ember }),
  sep,
  mi("Observed: each T3 instance", { dis: true }),
  mi("uses its own login", { dis: true }),
  sep,
  mi("Manage Accounts…"),
];
const macScene = `<div layer-name="macOS scene" style="display:flex;flex-direction:column;width:640px;flex-shrink:0;background:#D9DDDB;border-radius:10px;overflow:hidden;padding-bottom:28px">${menubar("Sun 18 Oct  9:41")}<div style="display:flex;align-items:flex-start;justify-content:flex-end;gap:2px;padding:4px 40px 0 0">${nmenu(250, subItems, "margin-top:30px")}${nmenu(310, mainItems)}</div></div>`;
const kmi = (label, { sub = false, dis = false } = {}) =>
  `<div style="display:flex;align-items:center;height:26px;padding:0 12px;${F}font-size:13px;color:${dis ? "#8C9190" : "#EDEFEE"}"><div style="flex:1;white-space:nowrap">${label}</div><div style="color:#8C9190">${sub ? "›" : ""}</div></div>`;
const ksep = `<div style="height:1px;background:rgba(255,255,255,0.10);margin:4px 0"></div>`;
const linScene = `<div layer-name="Linux scene" style="display:flex;flex-direction:column;justify-content:flex-end;width:300px;min-height:470px;background:#2A2E2D;border-radius:10px;overflow:hidden"><div style="display:flex;flex-direction:column;width:280px;margin:0 10px 6px auto;padding:4px 0;background:#353A39;border-radius:6px;box-shadow:0 8px 24px rgba(0,0,0,0.35)">${kmi("Relay — Running", { dis: true })}${ksep}${kmi("Claude Max · 3 observed", { sub: true })}${kmi("5h 48% left · wk 71% left", { dis: true })}${kmi("Codex Plus · 2 observed", { sub: true })}${kmi("5h 22% left · 1 stale", { dis: true })}${kmi("Anthropic API · 2 pooled", { sub: true })}${kmi("Tokens/min 76% left", { dis: true })}${ksep}${kmi("Open Quota Panel…")}${kmi("Settings…")}${kmi("Pause Routing")}${ksep}${kmi("Quit Relay")}</div><div style="display:flex;align-items:center;justify-content:flex-end;gap:12px;height:36px;padding:0 12px;background:#1E2120;${F}font-size:12px;color:#EDEFEE"><div style="display:flex;align-items:center;justify-content:center;width:26px;height:26px;border-radius:4px;background:rgba(255,255,255,0.12)">${trayIcon("#EDEFEE")}</div><div>09:41</div></div></div>`;
const html01 =
  title(
    "Relay · design review · surface 1 of 2",
    "Native tray menu",
    "The only surface guaranteed on macOS and Linux. Native menus render text rows only, so each group is one line and its accounts live in a submenu. Relay owns the words; the desktop owns the styling. Groups say how they route: <b>observed</b> (each T3 instance uses its own login) or <b>pooled</b> (Relay picks the account).",
  ) +
  `<div layer-name="Scenes" style="display:flex;gap:44px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:640px">${macScene}${cap("macOS · NSStatusItem via tray-icon", "Template icon, no title by default (optional: tightest fresh window of a pooled group). Menu text refreshes on reopen, not live while open.")}</div><div style="display:flex;flex-direction:column;gap:14px;width:300px">${linScene}${cap("Linux · StatusNotifierItem (ksni)", "Shell renders and places the menu (Plasma shown). GNOME needs the AppIndicator extension; without it Relay opens the quota panel at launch and says why.")}</div></div>`;

// ---------- board 03: expanded + wayland ----------
const footer03 = `<div style="display:flex;align-items:center;padding:10px 16px 12px;border-top:1px solid ${FR};${F}font-size:12px;color:${L}"><div style="flex:1">Today via Relay · <span style="color:${G};font-weight:500">1.24M</span> tokens</div><div style="color:#1F6B4F;font-weight:500">Settings…</div></div>`;
const pop03 = popShell(
  "Panel — expanded",
  header(true) +
    gClaude(true) +
    gCodex(false) +
    gApi(true) +
    nextRow +
    footer03,
);
const mac03 = `<div layer-name="macOS scene" style="display:flex;flex-direction:column;width:440px;flex-shrink:0;background:#D9DDDB;border-radius:10px;overflow:hidden;padding-bottom:28px">${menubar("9:41")}${pop03}</div>`;
const wl03 = (cloneId) =>
  `<div layer-name="Wayland scene" style="display:flex;flex-direction:column;width:480px;flex-shrink:0;background:#3A4541;border-radius:10px;overflow:hidden"><div style="display:flex;align-items:center;justify-content:space-between;height:26px;padding:0 12px;background:#141716;${F}font-size:12px;color:#EDEFEE"><div>Activities</div><div>Sun 09:41</div><div style="opacity:0.5">no tray icon</div></div><div style="display:flex;justify-content:center;padding:48px 0 56px"><div layer-name="Toplevel window" style="display:flex;flex-direction:column;width:340px;background:#FFFFFF;border-radius:12px;overflow:hidden;box-shadow:0 18px 44px rgba(0,0,0,0.40)"><div style="display:flex;align-items:center;height:36px;padding:0 10px;background:#F2F3F2;border-bottom:1px solid ${FR};${F}"><div style="width:24px"></div><div style="flex:1;text-align:center;font-size:13px;font-weight:600;color:${G}">Relay</div><div style="display:flex;align-items:center;justify-content:center;width:24px;height:24px;border-radius:12px;background:#E2E5E3;font-size:12px;color:${G}">✕</div></div><div style="padding:10px 14px;background:color-mix(in srgb, #B7791F 9%, white);border-bottom:1px solid ${FR};${F}font-size:11px;line-height:16px;color:#6E4A12">GNOME needs the AppIndicator extension to show Relay's tray icon. Until then this window is the way in.</div><x-paper-clone node-id="${cloneId}" style="width:340px;margin:0;border-radius:0;box-shadow:none" /></div></div></div>`;
const html03 = (cloneId) =>
  title(
    "Relay · design review · surface 2 of 2 (optional)",
    "Quota panel — expanded",
    "A small GPUI window opened from “Open Quota Panel…”. Groups are provider + plan + credential type. Each window is averaged only over accounts with fresh data; stale and unreported accounts are counted beside the average, never folded in. Only pooled groups have a “next” account.",
  ) +
  `<div layer-name="Scenes" style="display:flex;gap:64px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:440px">${mac03}${cap("macOS · anchored under the status item", "Opens below the icon like a popover; closes on focus loss. Rows expand in place; account cards show reset times and when each value was last seen.")}</div><div style="display:flex;flex-direction:column;gap:14px;width:480px">${wl03(cloneId)}${cap("Wayland · ordinary toplevel, compositor-placed", "Wayland gives clients no way to position a window beside a tray icon, so the panel is a normal window the compositor places (usually centred or where it was last). Same content; closes with its button or Esc. On X11 Relay may anchor near the pointer as best effort.")}</div></div>`;

// ---------- run ----------
await tools.paper.delete_nodes({ fileId, nodeIds: ["1-0", "2K-0", "6D-0"] });
const a1 = idOf(
  await tools.paper.create_artboard({
    fileId,
    name: "01 · Native tray menu — macOS & Linux",
    styles: boardStyles,
  }),
);
await tools.paper.write_html({
  fileId,
  targetNodeId: a1,
  mode: "insert-children",
  html: html01,
});
const a2 = idOf(
  await tools.paper.create_artboard({
    fileId,
    name: "02 · Quota panel — collapsed (macOS)",
    styles: {
      width: "420px",
      height: "fit-content",
      background: "#D9DDDB",
      display: "flex",
      flexDirection: "column",
      paddingBottom: "48px",
    },
  }),
);
const w2 = await tools.paper.write_html({
  fileId,
  targetNodeId: a2,
  mode: "insert-children",
  html: menubar("Sun 18 Oct  9:41") + pop02,
});
const popId = idOf(w2, "Popover");
const a3 = idOf(
  await tools.paper.create_artboard({
    fileId,
    name: "03 · Quota panel — expanded & Wayland",
    styles: boardStyles,
  }),
);
await tools.paper.write_html({
  fileId,
  targetNodeId: a3,
  mode: "insert-children",
  html: html03(popId),
});
return { a1, a2, a3, popId };
