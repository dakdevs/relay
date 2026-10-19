// Batch B (run after kit.js): boards 02 tray collapsed, 03 tray expanded + Linux, 10 Through Relay sparkline states.
const OLD = ["3L6-0", "3UF-0", "47P-0"];
const collapsed = (K, name, mode) =>
  K.pop(name, K.anthSection(false) + K.oaiSection(false), mode);
const html02 =
  title(
    "Relay · design review · tray",
    "Tray popover — collapsed",
    "One tonal section per provider, one group per comparable plan or credential, one bar per quota window. Accounts in use are named; no account is predicted. The pinned footer shows tokens through Relay with a week-over-week or month-over-month comparison, and Open Relay. Light and dark follow the system.",
  ) +
  `<div layer-name="Scenes" style="display:flex;gap:64px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:460px">${scene(Lk, collapsed(Lk, "Popover — light", "Week"))}${cap("Light · Week selected", "Open Relay brings up the main window on Quotas. Ⅱ pauses routing; the gear opens Settings. The sparkline compares this week so far with the same span of last week.")}</div><div style="display:flex;flex-direction:column;gap:14px;width:460px">${scene(Dk, collapsed(Dk, "Popover — dark", "Month"))}${cap("Dark · Month selected", "Same structure on dark tonal surfaces. Month compares Oct 1–18 09:41 with Sep 1–18 09:41 — equal elapsed time, not a full month.")}</div></div>`;
const linux = (cloneId) =>
  `<div layer-name="Linux scene" style="display:flex;flex-direction:column;width:480px;flex-shrink:0;background:#3A4541;border-radius:12px;overflow:hidden"><div style="display:flex;align-items:center;justify-content:flex-end;gap:12px;height:30px;padding:0 12px;background:#141716;${F}font-size:12px;color:#EDEFEE"><div style="display:flex;align-items:center;justify-content:center;width:24px;height:24px;border-radius:5px;background:rgba(255,255,255,0.14)">${Lk.trayIcon("#EDEFEE")}</div><div>Sun 09:41</div></div><div style="display:flex;justify-content:flex-end;padding:8px 10px 40px 0"><x-paper-clone node-id="${cloneId}" style="margin:0" /></div></div>`;
const html03 = (cloneId) =>
  title(
    "Relay · design review · tray",
    "Tray popover — expanded",
    "Groups expand inline into one card per account: your label, a masked identifier, last-seen time, every quota window with its own reset, reset credits when reported, and state tags. The footer stays pinned while the list scrolls.",
  ) +
  `<div layer-name="Scenes" style="display:flex;gap:56px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:460px">${scene(Lk, Lk.pop("Popover — expanded", Lk.anthSection(true, true) + Lk.oaiSection(true), "Week"))}${cap("macOS · anchored below the status item", "Keyboard focus on the Work card is a filled tint, not an outline. Esc or a click outside closes and returns focus to the previous app. Height follows content up to the screen; the account list scrolls under the pinned footer.")}</div><div style="display:flex;flex-direction:column;gap:14px;width:480px">${linux(cloneId)}${cap("Linux · same popover, solid surfaces, system UI font", "Left-click on the StatusNotifierItem opens it when the host sends Activate; right-click opens the native fallback menu (board 07). Anchored where layer-shell exists (KDE, wlroots; GPUI support unverified), compositor-placed on GNOME (AppIndicator extension required), click point on X11. No blur is promised.")}</div></div>`;

// ---------- 10 sparkline states ----------
const T = LT,
  K = Lk;
const card = (h, sub, inner) =>
  `<div style="display:flex;flex-direction:column;gap:12px;width:388px;padding:14px 16px;border-radius:14px;background:${T.surf}"><div style="display:flex;flex-direction:column;gap:2px;${F}"><div style="font-size:13px;font-weight:600;color:${T.fg}">${h}</div><div style="font-size:11px;line-height:16px;color:${T.mu}">${sub}</div></div>${inner}</div>`;
const numRow = ({ num, d, sub, spark }) =>
  `<div style="display:flex;align-items:flex-end;gap:10px;${F}"><div style="display:flex;flex-direction:column;gap:3px;flex:1"><div style="display:flex;align-items:baseline;gap:7px"><div style="font-size:19px;font-weight:700;letter-spacing:-0.02em;color:${T.fg}">${num}</div>${d}</div><div style="font-size:10px;line-height:14px;color:${T.mu}">${sub}</div></div>${spark}</div>`;
const tip = `<div style="display:flex;flex-direction:column;gap:3px;padding:8px 10px;border-radius:8px;background:${T.inset};${F}font-size:11px;color:${T.fg}"><div style="font-weight:600">Thu Oct 15</div><div style="display:flex;gap:12px;color:${T.mu}"><span><span style="color:${T.ev}">—</span> This week 1.6M</span><span>┄ Last week (Thu Oct 8) 1.2M</span></div></div>`;
const muted = (t) => K.tag(t, T.mu);
const cards = [
  card(
    "Week over week (default)",
    "This week so far vs the same elapsed span last week.",
    K.through("Week"),
  ),
  card(
    "Pointer hover or keyboard focus on a day",
    "Shows the day, both values and the matching prior day. Arrow keys move between days; the marker jumps, it does not chase.",
    K.through("Week", { marker: 3 }) + tip,
  ),
  card(
    "Month over month",
    "Oct 1–18 09:41 vs Sep 1–18 09:41. If last month is shorter (e.g. Mar 30 vs Feb), the span ends on its last day and says so.",
    K.through("Month"),
  ),
  card(
    "Partial current period · Monday 09:41",
    "One day of seven so far. The hollow end dot marks an unfinished bucket.",
    numRow({
      num: "0.2M",
      d: muted("Too early"),
      sub: "vs 0.3M · Mon 00:00–09:41 vs last Mon",
      spark: K.spark({ cur: [0.2], prior: [0.3] }),
    }),
  ),
  card(
    "No prior history",
    "Relay recorded nothing last week, so there is no comparison and no percentage.",
    numRow({
      num: fmtM(sum(WEEK.cur)),
      d: muted("No comparison"),
      sub: "Comparison starts Mon Oct 26",
      spark: K.spark({ prior: null }),
    }),
  ),
  card(
    "Insufficient prior history",
    "Last week only partly recorded (Relay started Thu Oct 8). The prior line starts there; no percentage is shown.",
    numRow({
      num: fmtM(sum(WEEK.cur)),
      d: muted("Prior incomplete"),
      sub: "vs 2.0M from Thu–Sun only",
      spark: K.spark({ prior: [null, null, null, 1.2, 1.0, 0.6, 0.2] }),
    }),
  ),
  card(
    "Missing data vs real zero",
    "Tue has no data (Relay was not running): the line breaks over a tinted band. Wed had zero tokens: plotted at the baseline with a dot.",
    numRow({
      num: "5.0M",
      d: K.tag("Incomplete", T.am),
      sub: "Tue missing · Wed 0 tokens",
      spark: K.spark({
        cur: [1.1, null, 0, 1.6, 1.3, 0.8, 0.2],
        gapAt: [1],
        zeroAt: [2],
      }),
    }),
  ),
  card(
    "No traffic yet",
    "Nothing has passed through Relay. No line is drawn; zero is not implied.",
    numRow({
      num: "—",
      d: "",
      sub: "No requests through Relay yet",
      spark: K.spark({
        cur: [null, null, null, null, null, null, null],
        prior: null,
        partial: false,
      }),
    }),
  ),
];
const defs = [
  [
    "Measured",
    "Input + output tokens of requests that passed through Relay, per local calendar bucket. Cached reads appear in Usage, not here. Not account totals and not quota.",
  ],
  [
    "Week",
    "Calendar week in the local time zone; first weekday follows the system locale (Mon here). Current span = week start → now. Prior span = last week start → same elapsed time.",
  ],
  [
    "Month",
    "Calendar month, same rule by elapsed time. When the prior month is shorter than today's day-of-month, the prior span ends on its last day and the label says so.",
  ],
  [
    "Buckets",
    "Daily buckets; the current bucket is partial (hollow end dot). Null = no data (gap); 0 = zero traffic (baseline dot).",
  ],
  [
    "Delta",
    "Shown only when both spans are fully recorded and the prior total is above zero; otherwise a muted state label.",
  ],
  [
    "Data flow",
    "Usage records are aggregated asynchronously into bounded per-day totals (current and previous period plus a small margin). The UI reads a snapshot; nothing here runs on the routing path or triggers network calls.",
  ],
  [
    "Accessibility",
    "“Tokens through Relay, week to date 7.4 million, up 17 percent versus 6.3 million over the same span last week.” Each day is focusable with the same values.",
  ],
];
const defsHtml = `<div style="display:flex;flex-direction:column;gap:8px;max-width:1240px;${F}">${defs.map(([k, v], i) => `<div style="display:flex;gap:20px;padding:10px 14px;border-radius:10px;${i % 2 === 0 ? `background:${T.inset};` : ""}"><div style="width:120px;flex-shrink:0;font-size:12px;font-weight:600;color:${G}">${k}</div><div style="flex:1;font-size:12px;line-height:18px;color:${L}">${v}</div></div>`).join("")}</div>`;
const html10 =
  title(
    "Relay · design review · behaviour spec",
    "Through Relay — sparkline states",
    "The tray footer and Usage chart share one component: current span solid, prior equivalent span dashed, one total and one delta. Design sample data. Quota bars and reset credits are unrelated and never drawn here.",
  ) +
  `<div layer-name="States" style="display:flex;flex-wrap:wrap;gap:16px;padding:20px;border-radius:16px;background:${T.canvas};max-width:1244px">${cards.join("")}</div>` +
  defsHtml;

// ---------- run ----------
await tools.paper.delete_nodes({ fileId, nodeIds: OLD });
const out = {};
const mk = async (key, name, html, w) => {
  out[key] = idOf(
    await tools.paper.create_artboard({ fileId, name, styles: board(w) }),
  );
  return tools.paper.write_html({
    fileId,
    targetNodeId: out[key],
    mode: "insert-children",
    html,
  });
};
const w2 = await mk(
  "a2",
  "02 · Tray popover — collapsed (light & dark)",
  html02,
  1120,
);
const cloneId = idOf(w2, "Popover — light");
await mk(
  "a3",
  "03 · Tray popover — expanded (macOS & Linux)",
  html03(cloneId),
  1120,
);
await mk("a10", "10 · Through Relay — sparkline states", html10, 1340);
const fix = JSON.stringify(
  await tools.paper.find_nodes({
    fileId,
    nodeId: "3AR-0",
    textValue: "provider exposes no credits",
  }),
);
const fixIds = [...fix.matchAll(/\\"id\\": \\"([0-9A-Z]+-[0-9]+)\\"/g)]
  .map((m) => m[1])
  .filter((x) => x !== "3AR-0");
if (fixIds.length)
  await tools.paper.set_text_content({
    fileId,
    updates: fixIds.map((nodeId) => ({ nodeId, textContent: "none exposed" })),
  });
const nodes = {};
nodes[out.a2] = [{ format: "png", scale: "2x" }];
nodes[out.a3] = [{ format: "png", scale: "2x" }];
nodes[out.a10] = [{ format: "png", scale: "1x" }];
const ex = await tools.paper.export({ fileId, type: "image", nodes });
return { ...out, paths: exportPaths(ex) };
