// Executor `execute` program: regenerates Relay boards 02 (tray popover, light + dark) and 03 (expanded + Linux).
// Keeps the approved quota-panel visual direction. Values are design examples only.
const fileId = "01M58Z151ZVWZ4VVK921R3Q5J9";
const F = `font-family:var(--font-ui);font-variant-numeric:tabular-nums;`;
const LIGHT = {
  bg: "#FFFFFF",
  fg: "#1C1F1E",
  mu: "#6B716E",
  tr: "#ECEEED",
  card: "#F7F8F7",
  band: "#FAFBFA",
  ev: "#1F6B4F",
  am: "#B7791F",
  em: "#B4432F",
  hatch: "#D5D9D7",
  desk: "#D9DDDB",
  bar: "rgba(255,255,255,0.55)",
  shadow: "0 14px 36px rgba(28,31,30,0.18),0 0 0 0.5px rgba(28,31,30,0.16)",
};
const DARK = {
  bg: "#232726",
  fg: "#EDEFEE",
  mu: "#9AA09D",
  tr: "#363B3A",
  card: "#2B302E",
  band: "#272B2A",
  ev: "#4DB389",
  am: "#D9A140",
  em: "#E26A52",
  hatch: "#4A504E",
  desk: "#141716",
  bar: "rgba(30,33,32,0.7)",
  shadow: "0 14px 36px rgba(0,0,0,0.5),0 0 0 0.5px rgba(255,255,255,0.12)",
};
const idOf = (res, name) => {
  const s = JSON.stringify(res);
  if (!name) return s.match(/\\"id\\": \\"([0-9A-Z]+-[0-9]+)\\"/)[1];
  const re = new RegExp(
    `\\\\"id\\\\": \\\\"([0-9A-Z]+-[0-9]+)\\\\",\\\\n\\s*\\\\"name\\\\": \\\\"${name}\\\\"`,
  );
  return s.match(re)[1];
};
const kit = (T) => {
  const level = (p) => (p < 15 ? T.em : p < 40 ? T.am : T.ev);
  const chev = (o) =>
    `<svg width="10" height="10" viewBox="0 0 10 10" style="flex-shrink:0;transform:rotate(${o ? 90 : 0}deg)"><path d="M3.5 2 L7 5 L3.5 8" fill="none" stroke="${T.mu}" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  const tag = (t, c) =>
    `<div style="flex-shrink:0;padding:1px 6px;border-radius:4px;background:color-mix(in srgb, ${c} 14%, transparent);${F}font-size:9px;font-weight:600;letter-spacing:0.07em;color:${c};white-space:nowrap">${t}</div>`;
  const win = ({
    label,
    pct,
    meta,
    stale = false,
    metaColor = T.mu,
    indent = 18,
  }) => {
    const track =
      pct == null
        ? `<div style="flex:1;height:4px;border-radius:3px;background-image:repeating-linear-gradient(135deg,${T.hatch} 0px,${T.hatch} 2px,transparent 2px,transparent 5px);background-color:${T.tr}"></div>`
        : `<div style="flex:1;height:4px;border-radius:3px;background:${T.tr};overflow:hidden;display:flex"><div style="width:${pct}%;height:4px;border-radius:3px;background:${level(pct)};opacity:${stale ? 0.35 : 1}"></div></div>`;
    const vcol = pct == null || stale ? T.mu : pct < 15 ? T.em : T.fg;
    return `<div style="display:flex;align-items:center;gap:10px;padding-left:${indent}px;${F}"><div style="width:52px;flex-shrink:0;font-size:10px;font-weight:500;letter-spacing:0.06em;color:${T.mu};text-transform:uppercase;white-space:nowrap">${label}</div>${track}<div style="width:38px;flex-shrink:0;text-align:right;font-size:13px;font-weight:500;color:${vcol}">${pct == null ? "—" : pct + "%"}</div><div style="width:78px;flex-shrink:0;text-align:right;font-size:11px;color:${metaColor};white-space:nowrap">${meta}</div></div>`;
  };
  const head = ({
    name,
    count,
    mode,
    open = false,
    note = "",
    noteColor = T.mu,
  }) =>
    `<div style="display:flex;align-items:center;gap:8px;${F}">${chev(open)}<div style="font-size:13px;font-weight:600;color:${T.fg};white-space:nowrap">${name}</div><div style="font-size:11px;color:${T.mu};white-space:nowrap">${count}</div>${mode === "pooled" ? tag("POOLED", T.ev) : tag("OBSERVED", T.mu)}<div style="flex:1"></div><div style="font-size:11px;font-weight:500;color:${noteColor};white-space:nowrap">${note}</div></div>`;
  const explain = (t) =>
    `<div style="padding-left:18px;${F}font-size:11px;line-height:16px;color:${T.mu}">${t}</div>`;
  const acct = ({ name, tagH = "", seen, seenColor = T.mu, rows, note = "" }) =>
    `<div style="display:flex;flex-direction:column;gap:7px;padding:9px 10px;margin-left:18px;border-radius:8px;background:${T.card}"><div style="display:flex;align-items:center;gap:6px;${F}"><div style="font-size:12px;font-weight:600;color:${T.fg}">${name}</div>${tagH}<div style="flex:1"></div><div style="font-size:11px;color:${seenColor};white-space:nowrap">${seen}</div></div>${rows.map((r) => win({ ...r, indent: 0 })).join("")}${note ? `<div style="${F}font-size:11px;line-height:16px;color:${T.mu}">${note}</div>` : ""}</div>`;
  const group = (h, rows) =>
    `<div style="display:flex;flex-direction:column;gap:8px;padding:12px 16px 14px;border-top:1px solid ${T.tr}">${h}${rows.join("")}</div>`;
  const header = (legend) =>
    `<div layer-name="Header" style="display:flex;align-items:center;gap:8px;padding:14px 16px ${legend ? 4 : 12}px;${F}"><div style="font-size:15px;font-weight:600;color:${T.fg};letter-spacing:-0.01em">Relay</div><div style="display:flex;align-items:center;gap:5px;padding:2px 8px 2px 6px;border-radius:10px;background:color-mix(in srgb, ${T.ev} 14%, transparent)"><div style="width:6px;height:6px;border-radius:3px;background:${T.ev}"></div><div style="font-size:11px;font-weight:500;color:${T.ev}">Running</div></div><div style="flex:1"></div><div style="font-size:11px;color:${T.mu}">127.0.0.1:8317</div></div>${legend ? `<div style="padding:0 16px 12px;${F}font-size:11px;color:${T.mu}">Bars show quota left · averages use fresh data (&lt; 15 min)</div>` : ""}`;
  const nextRow = `<div layer-name="Next route" style="display:flex;align-items:center;gap:8px;padding:10px 16px;border-top:1px solid ${T.tr};background:${T.band};${F}font-size:11px;color:${T.mu}"><div style="flex-shrink:0;text-transform:uppercase;letter-spacing:0.06em;font-weight:500;font-size:10px">New sessions</div><div style="flex-shrink:0;color:${T.fg};font-size:12px;white-space:nowrap">→ Claude · personal</div><div style="flex:1"></div><div style="flex-shrink:0;white-space:nowrap">3 sticky</div></div>`;
  const statusBar = `<div layer-name="Controls" style="display:flex;align-items:center;gap:8px;padding:10px 12px 12px 16px;border-top:1px solid ${T.tr};${F}"><div style="display:flex;flex-direction:column;gap:1px;flex:1"><div style="font-size:11px;color:${T.mu}">Today via Relay</div><div style="font-size:13px;color:${T.fg}"><span style="font-weight:600">1.24M</span> <span style="color:${T.mu};font-size:11px">tokens · 312 req</span></div></div>${["Usage", "Pause"].map((t) => `<div style="padding:4px 10px;border-radius:6px;background:${T.card};box-shadow:0 0 0 0.5px ${T.tr};font-size:12px;font-weight:500;color:${T.fg}">${t}</div>`).join("")}<div style="display:flex;align-items:center;justify-content:center;width:26px;height:26px;border-radius:6px;background:${T.card};box-shadow:0 0 0 0.5px ${T.tr}"><svg width="14" height="14" viewBox="0 0 14 14"><circle cx="7" cy="7" r="2.2" fill="none" stroke="${T.mu}" stroke-width="1.3"/><path d="M7 1.5v1.6M7 10.9v1.6M1.5 7h1.6M10.9 7h1.6M3.1 3.1l1.1 1.1M9.8 9.8l1.1 1.1M3.1 10.9l1.1-1.1M9.8 4.2l1.1-1.1" stroke="${T.mu}" stroke-width="1.3" stroke-linecap="round"/></svg></div></div>`;
  const pop = (name, inner, w = 340, extra = "margin:6px 16px 0 auto;") =>
    `<div layer-name="${name}" style="display:flex;flex-direction:column;width:${w}px;${extra}background:${T.bg};border-radius:12px;box-shadow:${T.shadow};overflow:hidden">${inner}</div>`;
  const trayIcon = (c) =>
    `<svg width="16" height="16" viewBox="0 0 16 16"><circle cx="3.5" cy="8" r="1.8" fill="${c}"/><path d="M5.5 8 H8 M8 8 C10 8 10 4 12.5 4 M8 8 C10 8 10 12 12.5 12" fill="none" stroke="${c}" stroke-width="1.4" stroke-linecap="round"/><circle cx="13" cy="4" r="1.3" fill="${c}"/><circle cx="13" cy="12" r="1.3" fill="${c}"/></svg>`;
  const menubar = (t) =>
    `<div style="display:flex;align-items:center;justify-content:flex-end;gap:14px;height:26px;padding:0 14px;background:${T.bar};${F}font-size:13px;color:${T.fg}"><div style="display:flex;align-items:center;justify-content:center;width:28px;height:22px;border-radius:5px;background:color-mix(in srgb, ${T.fg} 14%, transparent)">${trayIcon(T.fg)}</div><div>${t}</div></div>`;
  // groups
  const gClaude = (open) =>
    group(head({ name: "Claude", count: "Max · 3", mode: "pooled", open }), [
      win({ label: "5-hour", pct: 48, meta: "avg · 3 fresh" }),
      win({ label: "Weekly", pct: 71, meta: "avg · 3 fresh" }),
      ...(open
        ? [
            explain(
              "New sessions go to the account with the most headroom; running conversations stay where they started.",
            ),
            acct({
              name: "personal",
              tagH: tag("NEXT", T.ev),
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
              tagH: tag("EXHAUSTED", T.em),
              seen: "seen 1m ago",
              rows: [
                {
                  label: "5-hour",
                  pct: 0,
                  meta: "resets 38m",
                  metaColor: T.em,
                },
                { label: "Weekly", pct: 54, meta: "resets 2d" },
              ],
              note: "No new sessions until the 5-hour window resets.",
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
        noteColor: T.am,
      }),
      [
        win({ label: "5-hour", pct: 22, meta: "avg · 1 fresh" }),
        win({ label: "Weekly", pct: 64, meta: "avg · 1 fresh" }),
        ...(open
          ? [
              explain(
                "Observe mode: each T3 instance uses its own login; Relay only records usage and quota.",
              ),
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
                tagH: tag("STALE", T.am),
                seen: "seen 18m ago",
                seenColor: T.am,
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
  return {
    pop,
    header,
    gClaude,
    gCodex,
    nextRow,
    statusBar,
    menubar,
    trayIcon,
    T,
  };
};
const title = (eyebrow, h, body) =>
  `<div layer-name="Board title" style="display:flex;flex-direction:column;gap:6px;${F}"><div style="font-size:11px;font-weight:500;letter-spacing:0.08em;text-transform:uppercase;color:#1F6B4F">${eyebrow}</div><div style="font-size:30px;font-weight:700;letter-spacing:-0.02em;color:#1C1F1E">${h}</div><div style="font-size:14px;line-height:21px;color:#6B716E;max-width:820px">${body}</div></div>`;
const cap = (h, b) =>
  `<div style="display:flex;flex-direction:column;gap:4px;${F}"><div style="font-size:13px;font-weight:600;color:#1C1F1E">${h}</div><div style="font-size:12px;line-height:18px;color:#6B716E">${b}</div></div>`;
const scene = (K, inner, w = 440) =>
  `<div style="display:flex;flex-direction:column;width:${w}px;flex-shrink:0;background:${K.T.desk};border-radius:10px;overflow:hidden;padding-bottom:28px">${K.menubar("Sun 18 Oct  9:41")}${inner}</div>`;
const boardStyles = {
  width: "1080px",
  height: "fit-content",
  background: "#FFFFFF",
  display: "flex",
  flexDirection: "column",
  gap: "32px",
  padding: "48px",
};
const Lk = kit(LIGHT),
  Dk = kit(DARK);
const collapsed = (K, name) =>
  K.pop(
    name,
    K.header(false) +
      K.gClaude(false) +
      K.gCodex(false) +
      K.nextRow +
      K.statusBar,
  );
const html02 =
  title(
    "Relay · design review · primary surface",
    "Tray popover — collapsed",
    "Click the tray icon and this opens: one row per subscription group, one bar per quota window, the account new sessions will use, and three quiet controls. Light and dark follow the system appearance.",
  ) +
  `<div layer-name="Scenes" style="display:flex;gap:64px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:440px">${scene(Lk, collapsed(Lk, "Popover — light"))}${cap("Light", "Usage opens the analytics window; Pause stops routing new sessions; the gear opens Settings. Click a group to expand it in place.")}</div><div style="display:flex;flex-direction:column;gap:14px;width:440px">${scene(Dk, collapsed(Dk, "Popover — dark"))}${cap("Dark", "Same layout; accents are lifted for contrast on the dark surface. Stale, unknown and exhausted keep the same shapes, not just colours.")}</div></div>`;
const expanded = (K) =>
  K.pop(
    "Popover — expanded",
    K.header(true) + K.gClaude(true) + K.gCodex(true) + K.nextRow + K.statusBar,
  );
const linux = (cloneId) =>
  `<div layer-name="Linux scene" style="display:flex;flex-direction:column;width:480px;flex-shrink:0;background:#3A4541;border-radius:10px;overflow:hidden"><div style="display:flex;align-items:center;justify-content:flex-end;gap:12px;height:30px;padding:0 12px;background:#141716;${F}font-size:12px;color:#EDEFEE"><div style="display:flex;align-items:center;justify-content:center;width:24px;height:24px;border-radius:4px;background:rgba(255,255,255,0.14)">${Lk.trayIcon("#EDEFEE")}</div><div>Sun 09:41</div></div><div style="display:flex;justify-content:flex-end;padding:8px 10px 40px 0"><div layer-name="Linux popover" style="display:flex;flex-direction:column;width:340px;border-radius:12px;overflow:hidden;box-shadow:0 18px 44px rgba(0,0,0,0.40)"><x-paper-clone node-id="${cloneId}" style="width:340px;margin:0;border-radius:0;box-shadow:none" /></div></div></div>`;
const html03 = (cloneId) =>
  title(
    "Relay · design review · primary surface",
    "Tray popover — expanded",
    "Groups expand inline into account cards with their own bars, reset times and freshness. Pooled groups pick an account for each new session; observed groups only report. Exhausted, stale and unreported values keep distinct shapes.",
  ) +
  `<div layer-name="Scenes" style="display:flex;gap:64px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:440px">${scene(Lk, expanded(Lk))}${cap("macOS · anchored below the status item", "Borderless GPUI window positioned from the status item's frame; closes on focus loss or Esc. Height grows with expanded groups and scrolls past the screen.")}</div><div style="display:flex;flex-direction:column;gap:14px;width:480px">${linux(cloneId)}${cap("Linux · same popover, compositor-dependent placement", "Left-click on the StatusNotifierItem opens the popover; right-click shows the native fallback menu (board 07). KDE and wlroots shells: anchored to the panel corner where layer-shell is available. GNOME (AppIndicator extension required): opens as a small undecorated window the compositor places, usually centred. X11: placed at the reported click point.")}</div></div>`;
// run
await tools.paper.delete_nodes({ fileId, nodeIds: ["I7-0", "L7-0"] });
const a2 = idOf(
  await tools.paper.create_artboard({
    fileId,
    name: "02 · Tray popover — collapsed (light & dark)",
    styles: boardStyles,
  }),
);
const w2 = await tools.paper.write_html({
  fileId,
  targetNodeId: a2,
  mode: "insert-children",
  html: html02,
});
const cloneId = idOf(w2, "Popover — light");
const a3 = idOf(
  await tools.paper.create_artboard({
    fileId,
    name: "03 · Tray popover — expanded (macOS & Linux)",
    styles: boardStyles,
  }),
);
await tools.paper.write_html({
  fileId,
  targetNodeId: a3,
  mode: "insert-children",
  html: html03(cloneId),
});
return { a2, a3, cloneId };
