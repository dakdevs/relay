// Executor `execute` program: (re)generates Relay review boards 02–06 and 08 in Paper.
// Paper design tooling only — not application code. Values and identities are design examples.
const fileId = "01M58Z151ZVWZ4VVK921R3Q5J9";
const OLD = ["U5-0", "ZC-0", "17V-0", "1DB-0", "1K8-0"]; // boards replaced by this run
const F = `font-family:var(--font-ui);font-variant-numeric:tabular-nums;`;
const LIGHT = { bg: "#FFFFFF", fg: "#1C1F1E", mu: "#6B716E", tr: "#ECEEED", card: "#F7F8F7", band: "#FAFBFA", ev: "#1F6B4F", am: "#9A6614", em: "#B4432F", hatch: "#D5D9D7", desk: "#D9DDDB", bar: "rgba(255,255,255,0.55)", shadow: "0 14px 36px rgba(28,31,30,0.18),0 0 0 0.5px rgba(28,31,30,0.16)", anth: "#C96442", oai: "#111111", oaiFg: "#FFFFFF" };
const DARK = { bg: "#232726", fg: "#EDEFEE", mu: "#A3A9A6", tr: "#363B3A", card: "#2B302E", band: "#272B2A", ev: "#4DB389", am: "#D9A140", em: "#E26A52", hatch: "#4A504E", desk: "#141716", bar: "rgba(30,33,32,0.7)", shadow: "0 14px 36px rgba(0,0,0,0.5),0 0 0 0.5px rgba(255,255,255,0.12)", anth: "#D97757", oai: "#F2F2F2", oaiFg: "#111111" };
const idOf = (res, name) => {
  const s = JSON.stringify(res);
  if (!name) return s.match(/\\"id\\": \\"([0-9A-Z]+-[0-9]+)\\"/)[1];
  const re = new RegExp(`\\\\"id\\\\": \\\\"([0-9A-Z]+-[0-9]+)\\\\",\\\\n\\s*\\\\"name\\\\": \\\\"${name}\\\\"`);
  return s.match(re)[1];
};

// ---------- popover kit ----------
const kit = (T) => {
  const level = (p) => (p < 15 ? T.em : p < 40 ? T.am : T.ev);
  const chev = (o) => `<svg width="10" height="10" viewBox="0 0 10 10" style="flex-shrink:0;transform:rotate(${o ? 90 : 0}deg)"><path d="M3.5 2 L7 5 L3.5 8" fill="none" stroke="${T.mu}" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  const tag = (t, c) => `<div style="flex-shrink:0;width:fit-content;padding:1px 6px;border-radius:4px;background:color-mix(in srgb, ${c} 14%, transparent);${F}font-size:9px;font-weight:600;letter-spacing:0.06em;color:${c};white-space:nowrap">${t}</div>`;
  const tile = (p, size = 16) => `<div style="width:${size}px;height:${size}px;flex-shrink:0;border-radius:${Math.round(size / 4)}px;background:${p === "anth" ? T.anth : T.oai};display:flex;align-items:center;justify-content:center;${F}font-size:${Math.round(size * 0.56)}px;font-weight:700;color:${p === "anth" ? "#FFFFFF" : T.oaiFg}">${p === "anth" ? "A" : "O"}</div>`;
  const provider = (p, name) => `<div style="display:flex;align-items:center;gap:7px;padding:12px 16px 2px;border-top:1px solid ${T.tr};${F}">${tile(p)}<div style="font-size:12px;font-weight:600;letter-spacing:0.01em;color:${T.fg}">${name}</div></div>`;
  const win = ({ label, pct, meta, stale = false, metaColor = T.mu, indent = 18 }) => {
    const track = pct == null
      ? `<div style="flex:1;height:4px;border-radius:3px;background-image:repeating-linear-gradient(135deg,${T.hatch} 0px,${T.hatch} 2px,transparent 2px,transparent 5px);background-color:${T.tr}"></div>`
      : `<div style="flex:1;height:4px;border-radius:3px;background:${T.tr};overflow:hidden;display:flex"><div style="width:${pct}%;height:4px;border-radius:3px;background:${level(pct)};opacity:${stale ? 0.35 : 1}"></div></div>`;
    const vcol = pct == null || stale ? T.mu : pct < 15 ? T.em : T.fg;
    return `<div style="display:flex;align-items:center;gap:10px;padding-left:${indent}px;${F}"><div style="width:62px;flex-shrink:0;font-size:10px;font-weight:500;letter-spacing:0.05em;color:${T.mu};text-transform:uppercase;white-space:nowrap">${label}</div>${track}<div style="width:38px;flex-shrink:0;text-align:right;font-size:13px;font-weight:500;color:${vcol}">${pct == null ? "—" : pct + "%"}</div><div style="width:80px;flex-shrink:0;text-align:right;font-size:11px;color:${metaColor};white-space:nowrap">${meta}</div></div>`;
  };
  const head = ({ name, count, mode, open = false, note = "", noteColor = T.mu, focus = false }) => `<div style="display:flex;align-items:center;gap:8px;min-height:24px;margin:0 -6px;padding:0 6px;border-radius:6px;${focus ? `box-shadow:0 0 0 2px ${T.bg},0 0 0 4px #2F6FDB;` : ""}${F}">${chev(open)}<div style="font-size:13px;font-weight:600;color:${T.fg};white-space:nowrap">${name}</div><div style="font-size:11px;color:${T.mu};white-space:nowrap">${count}</div>${mode === "pooled" ? tag("POOLED", T.ev) : tag("OBSERVED", T.mu)}<div style="flex:1"></div><div style="font-size:11px;font-weight:500;color:${noteColor};white-space:nowrap">${note}</div></div>`;
  // collapsed identity line: which account takes new sessions, which are in use
  const who = (items) => `<div style="display:flex;align-items:center;gap:14px;padding-left:18px;${F}font-size:11px;color:${T.mu}">${items.map(([g, gc, name, role]) => `<div style="display:flex;align-items:center;gap:4px;white-space:nowrap"><span style="color:${gc};font-weight:700">${g}</span><span style="color:${T.fg};font-weight:600">${name}</span><span>${role}</span></div>`).join("")}</div>`;
  const explain = (t) => `<div style="padding-left:18px;${F}font-size:11px;line-height:16px;color:${T.mu}">${t}</div>`;
  const acct = ({ label, ident, seen, seenColor = T.mu, tags = "", rows, note = "", focus = false }) => `<div style="display:flex;flex-direction:column;gap:7px;padding:9px 10px;margin-left:18px;border-radius:8px;background:${T.card};${focus ? `box-shadow:0 0 0 2px ${T.bg},0 0 0 4px #2F6FDB;` : ""}"><div style="display:flex;align-items:flex-start;gap:6px;${F}"><div style="display:flex;flex-direction:column;gap:1px;flex:1"><div style="font-size:12px;font-weight:600;color:${T.fg}">${label}</div><div style="font-size:11px;color:${T.mu};white-space:nowrap">${ident} · <span style="color:${seenColor}">${seen}</span></div></div><div style="display:flex;gap:4px;padding-top:1px">${tags}</div></div>${rows.map((r) => win({ ...r, indent: 0 })).join("")}${note ? `<div style="${F}font-size:11px;line-height:16px;color:${T.mu}">${note}</div>` : ""}</div>`;
  const group = (h, rows) => `<div style="display:flex;flex-direction:column;gap:8px;padding:8px 16px 12px">${h}${rows.join("")}</div>`;
  const header = (legend) => `<div layer-name="Header" style="display:flex;align-items:center;gap:8px;padding:14px 16px ${legend ? 4 : 12}px;${F}"><div style="font-size:15px;font-weight:600;color:${T.fg};letter-spacing:-0.01em">Relay</div><div style="display:flex;align-items:center;gap:5px;padding:2px 8px 2px 6px;border-radius:10px;background:color-mix(in srgb, ${T.ev} 14%, transparent)"><div style="width:6px;height:6px;border-radius:3px;background:${T.ev}"></div><div style="font-size:11px;font-weight:500;color:${T.ev}">Running</div></div><div style="flex:1"></div><div style="font-size:11px;color:${T.mu}">127.0.0.1:8317</div></div>${legend ? `<div style="padding:0 16px 12px;${F}font-size:11px;color:${T.mu}">Bars show quota left · averages use fresh data (&lt; 15 min)</div>` : ""}`;
  const iconBtn = (svg) => `<div style="display:flex;align-items:center;justify-content:center;width:28px;height:28px;flex-shrink:0;border-radius:6px;background:${T.card};box-shadow:0 0 0 0.5px ${T.tr}">${svg}</div>`;
  const pauseSvg = `<svg width="12" height="12" viewBox="0 0 12 12"><rect x="3" y="2.5" width="2" height="7" rx="0.8" fill="${T.mu}"/><rect x="7" y="2.5" width="2" height="7" rx="0.8" fill="${T.mu}"/></svg>`;
  const gearSvg = `<svg width="14" height="14" viewBox="0 0 14 14"><circle cx="7" cy="7" r="2.2" fill="none" stroke="${T.mu}" stroke-width="1.3"/><path d="M7 1.5v1.6M7 10.9v1.6M1.5 7h1.6M10.9 7h1.6M3.1 3.1l1.1 1.1M9.8 9.8l1.1 1.1M3.1 10.9l1.1-1.1M9.8 4.2l1.1-1.1" stroke="${T.mu}" stroke-width="1.3" stroke-linecap="round"/></svg>`;
  const controls = `<div layer-name="Footer (pinned)" style="display:flex;align-items:center;gap:6px;padding:10px 12px 12px 16px;border-top:1px solid ${T.tr};background:${T.bg};${F}"><div style="display:flex;flex-direction:column;gap:1px;flex:1"><div style="font-size:11px;color:${T.mu}">Today via Relay</div><div style="font-size:13px;color:${T.fg}"><span style="font-weight:600">1.24M</span> <span style="color:${T.mu};font-size:11px">tokens · 312 req</span></div></div>${iconBtn(pauseSvg)}${iconBtn(gearSvg)}<div layer-name="Open Relay" style="display:flex;align-items:center;gap:6px;height:28px;padding:0 12px;border-radius:6px;background:${T.ev};font-size:12px;font-weight:600;color:${T.bg === "#FFFFFF" ? "#FFFFFF" : "#0F1A15"};white-space:nowrap">Open Relay<span style="font-weight:500;opacity:0.85">↗</span></div></div>`;
  const pop = (name, inner) => `<div layer-name="${name}" style="display:flex;flex-direction:column;width:360px;margin:6px 16px 0 auto;background:${T.bg};border-radius:12px;box-shadow:${T.shadow};overflow:hidden">${inner}</div>`;
  const trayIcon = (c) => `<svg width="16" height="16" viewBox="0 0 16 16"><circle cx="3.5" cy="8" r="1.8" fill="${c}"/><path d="M5.5 8 H8 M8 8 C10 8 10 4 12.5 4 M8 8 C10 8 10 12 12.5 12" fill="none" stroke="${c}" stroke-width="1.4" stroke-linecap="round"/><circle cx="13" cy="4" r="1.3" fill="${c}"/><circle cx="13" cy="12" r="1.3" fill="${c}"/></svg>`;
  const menubar = (t) => `<div style="display:flex;align-items:center;justify-content:flex-end;gap:14px;height:26px;padding:0 14px;background:${T.bar};${F}font-size:13px;color:${T.fg}"><div style="display:flex;align-items:center;justify-content:center;width:28px;height:22px;border-radius:5px;background:color-mix(in srgb, ${T.fg} 14%, transparent)">${trayIcon(T.fg)}</div><div>${t}</div></div>`;
  const NEXT = tag("→ NEXT", T.ev), INUSE = (n) => tag(`● IN USE · ${n}`, T.fg), EXH = tag("! EXHAUSTED", T.em), STALE = tag("◷ STALE", T.am);
  // ---- groups ----
  const claude = (open, focusAcct = false) => group(head({ name: "Claude Max", count: "3 subscriptions", mode: "pooled", open }), [
    win({ label: "5-hour", pct: 48, meta: "avg · 3 fresh" }), win({ label: "Weekly", pct: 71, meta: "avg · 3 fresh" }),
    ...(open ? [
      win({ label: "Fable 7d", pct: 97, meta: "avg · 3 fresh" }),
      explain("New sessions go to the account with the most headroom; running sessions stay where they started."),
      acct({ label: "Personal", ident: "d•••@gmail.com", seen: "seen 2m ago", tags: NEXT, rows: [{ label: "5-hour", pct: 81, meta: "resets 2h 10m" }, { label: "Weekly", pct: 88, meta: "resets 4d" }, { label: "Fable 7d", pct: 100, meta: "resets 4d" }], note: "Reset credits: 1 · expire Oct 22, 09:00" }),
      acct({ label: "Work", ident: "d•••@acme.co", seen: "seen 4m ago", tags: INUSE(2), focus: focusAcct, rows: [{ label: "5-hour", pct: 64, meta: "resets 3h" }, { label: "Weekly", pct: 70, meta: "resets 5d" }, { label: "Fable 7d", pct: 92, meta: "resets 5d" }] }),
      acct({ label: "Secondary", ident: "d•••@proton.me", seen: "seen 1m ago", tags: EXH, rows: [{ label: "5-hour", pct: 0, meta: "resets 38m", metaColor: T.em }, { label: "Weekly", pct: 54, meta: "resets 6d" }, { label: "Fable 7d", pct: 100, meta: "resets 6d" }], note: "Back in 38m · no new sessions until then." }),
    ] : [who([["→", T.ev, "Personal", "next"], ["●", T.fg, "Work", "2 in use"]])])]);
  const apiKeys = (open) => group(head({ name: "API keys", count: "2 keys", mode: "pooled", open }), [
    win({ label: "Tokens/min", pct: 76, meta: "avg · 2 fresh" }),
    ...(open ? [] : [who([["→", T.ev, "Key A", "next"]])])]);
  const codex = (open, focusHead = false) => group(head({ name: "Codex", count: "ChatGPT Plus · 2", mode: "observed", open, note: "◷ 1 stale", noteColor: T.am, focus: focusHead }), [
    win({ label: "5-hour", pct: 22, meta: "avg · 1 fresh" }), win({ label: "Weekly", pct: 64, meta: "avg · 1 fresh" }),
    ...(open ? [
      explain("Observe mode: each T3 instance uses its own login; Relay only records usage and quota."),
      acct({ label: "Dev", ident: "d•••@acme.co", seen: "seen 3m ago", tags: INUSE(1), rows: [{ label: "5-hour", pct: 22, meta: "resets 1h 05m" }, { label: "Weekly", pct: 64, meta: "resets 6d" }] }),
      acct({ label: "Team", ident: "t•••@acme.co", seen: "seen 18m ago", seenColor: T.am, tags: STALE, rows: [{ label: "5-hour", pct: 58, stale: true, meta: "not in avg" }, { label: "Weekly", pct: null, meta: "not reported" }], note: "Updates on this instance's next request." }),
    ] : [who([["●", T.fg, "Dev", "1 in use"]])])]);
  return { T, iconBtn, pauseSvg, gearSvg, tag, tile, provider, pop, header, controls, menubar, trayIcon, claude, apiKeys, codex, NEXT, INUSE, EXH, STALE };
};

// ---------- board chrome ----------
const G = "#1C1F1E", L = "#6B716E", FR = "#ECEEED", EV = "#1F6B4F", AM = "#9A6614", EM = "#B4432F";
const title = (eyebrow, h, body) => `<div layer-name="Board title" style="display:flex;flex-direction:column;gap:6px;${F}"><div style="font-size:11px;font-weight:500;letter-spacing:0.08em;text-transform:uppercase;color:${EV}">${eyebrow}</div><div style="font-size:30px;font-weight:700;letter-spacing:-0.02em;color:${G}">${h}</div><div style="font-size:14px;line-height:21px;color:${L};max-width:840px">${body}</div></div>`;
const cap = (h, b) => `<div style="display:flex;flex-direction:column;gap:4px;${F}"><div style="font-size:13px;font-weight:600;color:${G}">${h}</div><div style="font-size:12px;line-height:18px;color:${L}">${b}</div></div>`;
const scene = (K, inner, w = 460) => `<div style="display:flex;flex-direction:column;width:${w}px;flex-shrink:0;background:${K.T.desk};border-radius:10px;overflow:hidden;padding-bottom:28px">${K.menubar("Sun 18 Oct  9:41")}${inner}</div>`;
const board = (w = 1080) => ({ width: `${w}px`, height: "fit-content", background: "#FFFFFF", display: "flex", flexDirection: "column", gap: "32px", padding: "48px" });
const Lk = kit(LIGHT), Dk = kit(DARK);
const brandNote = "Monogram tiles are placeholders in each provider's brand colour; the shipped app uses official marks only where the providers' brand guidelines allow, otherwise these text wordmarks.";

// ---------- 02 collapsed ----------
const collapsed = (K, name) => K.pop(name, K.header(false) + K.provider("anth", "Anthropic") + K.claude(false) + K.apiKeys(false) + K.provider("oai", "OpenAI") + K.codex(false) + K.controls);
const html02 = title("Relay · design review · primary surface", "Tray popover — collapsed",
  "Grouped by provider, then by comparable plan or credential. Each group keeps its own quota windows, and a one-line identity row names the account that will take new sessions (→) and the accounts in use now (●). Light and dark follow the system appearance.")
  + `<div layer-name="Scenes" style="display:flex;gap:64px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:460px">${scene(Lk, collapsed(Lk, "Popover — light"))}${cap("Light", "Open Relay brings up the main window on Quotas. Pause (Ⅱ) stops placing new sessions; the gear opens Settings. Click or press Space on a group to expand it in place. " + brandNote)}</div><div style="display:flex;flex-direction:column;gap:14px;width:460px">${scene(Dk, collapsed(Dk, "Popover — dark"))}${cap("Dark", "Same structure; accents lifted for contrast. Status always pairs a glyph and a word with its colour, so it reads without colour.")}</div></div>`;

// ---------- 03 expanded ----------
const expanded = (K) => K.pop("Popover — expanded", K.header(true) + K.provider("anth", "Anthropic") + K.claude(true, true) + K.apiKeys(false) + K.provider("oai", "OpenAI") + K.codex(true) + K.controls);
const linux = (cloneId) => `<div layer-name="Linux scene" style="display:flex;flex-direction:column;width:480px;flex-shrink:0;background:#3A4541;border-radius:10px;overflow:hidden"><div style="display:flex;align-items:center;justify-content:flex-end;gap:12px;height:30px;padding:0 12px;background:#141716;${F}font-size:12px;color:#EDEFEE"><div style="display:flex;align-items:center;justify-content:center;width:24px;height:24px;border-radius:4px;background:rgba(255,255,255,0.14)">${Lk.trayIcon("#EDEFEE")}</div><div>Sun 09:41</div></div><div style="display:flex;justify-content:flex-end;padding:8px 10px 40px 0"><div layer-name="Linux popover" style="display:flex;flex-direction:column;width:360px;border-radius:12px;overflow:hidden;box-shadow:0 18px 44px rgba(0,0,0,0.40)"><x-paper-clone node-id="${cloneId}" style="width:360px;margin:0;border-radius:0;box-shadow:none" /></div></div></div>`;
const html03 = (cloneId) => title("Relay · design review · primary surface", "Tray popover — expanded",
  "Groups expand inline into one card per account: a stable label you chose, a masked identifier, last-seen time, and state tags. “→ Next” (chosen for the next new session) and “● In use” (carrying sessions now) are separate tags, because they are often different accounts.")
  + `<div layer-name="Scenes" style="display:flex;gap:56px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:460px">${scene(Lk, expanded(Lk))}${cap("macOS · anchored below the status item", "The footer with Open Relay stays pinned while the account list scrolls. Groups show their model-specific window only when expanded. Keyboard focus ring shown on the Work card. Esc or a click outside closes the popover and returns focus to the previous app.")}</div><div style="display:flex;flex-direction:column;gap:14px;width:480px">${linux(cloneId)}${cap("Linux · same popover, solid surface, system UI font", "Left-click on the StatusNotifierItem opens it when the host sends Activate; right-click opens the native fallback menu (board 07). Placement depends on the compositor: anchored where layer-shell is available (KDE, wlroots; GPUI support unverified), compositor-placed on GNOME (AppIndicator extension required), click point on X11. No blur is promised on Linux.")}</div></div>`;

// ---------- 04 accounts ----------
const A = kit(LIGHT);
const btn = (t, primary = false) => `<div style="display:flex;align-items:center;height:28px;padding:0 12px;border-radius:6px;${primary ? `background:${EV};color:#FFFFFF;` : `background:#FFFFFF;color:${G};box-shadow:0 0 0 0.5px rgba(0,0,0,0.2),0 1px 1px rgba(0,0,0,0.06);`}${F}font-size:12px;font-weight:500;white-space:nowrap">${t}</div>`;
const lights = `<div style="display:flex;gap:8px;width:60px">${["#FF5F57", "#FEBC2E", "#28C840"].map((c) => `<div style="width:12px;height:12px;border-radius:6px;background:${c}"></div>`).join("")}</div>`;
const tabs = (active) => `<div style="display:flex;padding:2px;border-radius:7px;background:#E4E7E5">${["Quotas", "Accounts", "Usage", "Routing"].map((t) => `<div style="padding:3px 14px;border-radius:5px;${t === active ? "background:#FFFFFF;box-shadow:0 1px 2px rgba(0,0,0,0.12);" : ""}${F}font-size:12px;font-weight:${t === active ? 600 : 500};color:${t === active ? G : L}">${t}</div>`).join("")}</div>`;
const windowChrome = (name, active, body, w = 720) => `<div layer-name="${name}" style="display:flex;flex-direction:column;width:${w}px;flex-shrink:0;background:#FFFFFF;border-radius:10px;overflow:hidden;box-shadow:0 18px 44px rgba(28,31,30,0.18),0 0 0 0.5px rgba(28,31,30,0.2)"><div style="display:flex;align-items:center;height:44px;padding:0 14px;background:#F4F5F4;border-bottom:1px solid ${FR}">${lights}<div style="flex:1;display:flex;justify-content:center">${tabs(active)}</div><div style="width:60px"></div></div>${body}</div>`;
const sectionLabel = (t) => `<div style="${F}font-size:10px;font-weight:600;letter-spacing:0.08em;text-transform:uppercase;color:${L}">${t}</div>`;
const provRow = (p, name) => `<div style="display:flex;align-items:center;gap:8px;height:30px;padding:0 20px;background:#FAFBFA;border-bottom:1px solid ${FR};${F}">${A.tile(p)}<div style="font-size:12px;font-weight:600;color:${G}">${name}</div></div>`;
const aRow = ({ label, ident, plan, mode, tags, status, sColor = L, action = "" }) => `<div style="display:flex;align-items:center;gap:12px;height:46px;padding:0 20px 0 44px;border-bottom:1px solid ${FR};${F}font-size:13px;color:${G}"><div style="width:150px;flex-shrink:0;display:flex;flex-direction:column;gap:1px"><div style="font-weight:600">${label}</div><div style="font-size:11px;color:${L}">${ident}</div></div><div style="width:120px;flex-shrink:0;font-size:12px;color:${L}">${plan}</div><div style="width:76px;flex-shrink:0">${mode === "pooled" ? A.tag("POOLED", EV) : A.tag("OBSERVED", L)}</div><div style="flex:1;display:flex;flex-direction:column;gap:3px"><div style="display:flex;gap:4px">${tags}</div><div style="font-size:11px;color:${sColor};white-space:nowrap">${status}</div></div><div style="width:86px;flex-shrink:0;display:flex;justify-content:flex-end">${action}</div></div>`;
const accountsBody = `<div style="display:flex;flex-direction:column"><div style="display:flex;align-items:center;gap:10px;padding:16px 20px 12px;${F}"><div style="font-size:15px;font-weight:600;color:${G}">Accounts</div><div style="font-size:12px;color:${L}">8 accounts · 2 providers</div><div style="flex:1"></div>${btn("Connect…", true)}</div>
<div style="display:flex;align-items:center;gap:12px;height:26px;padding:0 20px 0 44px;border-top:1px solid ${FR};border-bottom:1px solid ${FR};${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="width:150px">Account</div><div style="width:120px">Plan · credential</div><div style="width:76px">Mode</div><div style="flex:1">State</div><div style="width:86px"></div></div>
${provRow("anth", "Anthropic")}
${aRow({ label: "Personal", ident: "d•••@gmail.com", plan: "Claude Max · subscription", mode: "pooled", tags: A.NEXT, status: "Healthy · seen 2m ago" })}
${aRow({ label: "Work", ident: "d•••@acme.co", plan: "Claude Max · subscription", mode: "pooled", tags: A.INUSE(2), status: "Healthy · seen 4m ago" })}
${aRow({ label: "Secondary", ident: "d•••@proton.me", plan: "Claude Max · subscription", mode: "pooled", tags: A.EXH, status: "5-hour resets in 38m", sColor: EM })}
${aRow({ label: "Old work", ident: "d•••@oldco.io", plan: "Claude Pro · subscription", mode: "observed", tags: A.tag("○ DISCONNECTED", L), status: "T3 instance not found", action: btn("Reconnect") })}
${aRow({ label: "Key A", ident: "sk-ant-…7f2c", plan: "API key", mode: "pooled", tags: A.tag("→ NEXT", EV), status: "Healthy · seen 20s ago" })}
${aRow({ label: "Key B", ident: "sk-ant-…91ab", plan: "API key", mode: "pooled", tags: A.tag("Ⅱ PAUSED", L), status: "Paused by you · not routed", action: btn("Resume") })}
${provRow("oai", "OpenAI")}
${aRow({ label: "Dev", ident: "d•••@acme.co", plan: "ChatGPT Plus · Codex", mode: "observed", tags: A.INUSE(1), status: "Healthy · seen 3m ago" })}
${aRow({ label: "Team", ident: "t•••@acme.co", plan: "ChatGPT Plus · Codex", mode: "observed", tags: A.STALE, status: "Last seen 18m ago", sColor: AM })}
<div style="padding:12px 20px 16px;${F}font-size:11px;line-height:16px;color:${L}">Labels are yours and stay fixed; identifiers are masked. Relay never reads or changes T3's saved logins, and counts only traffic that passes through Relay.</div></div>`;
const choice = ({ titleT, body, chip, chipC, selected = false }) => `<div style="display:flex;gap:12px;padding:12px 14px;border-radius:8px;${selected ? `box-shadow:0 0 0 1.5px ${EV};background:color-mix(in srgb, ${EV} 4%, white);` : `box-shadow:0 0 0 1px ${FR};`}"><div style="width:14px;height:14px;flex-shrink:0;margin-top:2px;border-radius:7px;${selected ? `border:4px solid ${EV};width:6px;height:6px;` : `border:1.5px solid #B9BEBB;width:11px;height:11px;`}"></div><div style="display:flex;flex-direction:column;gap:4px;flex:1;${F}"><div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap"><div style="font-size:13px;font-weight:600;color:${G}">${titleT}</div>${A.tag(chip, chipC)}</div><div style="font-size:12px;line-height:17px;color:${L}">${body}</div></div></div>`;
const sheet = (name, heading, step, inner, footer) => `<div layer-name="${name}" style="display:flex;flex-direction:column;width:460px;flex-shrink:0;background:#FFFFFF;border-radius:12px;overflow:hidden;box-shadow:0 22px 50px rgba(28,31,30,0.22),0 0 0 0.5px rgba(28,31,30,0.2)"><div style="display:flex;flex-direction:column;gap:3px;padding:18px 20px 14px;${F}"><div style="font-size:11px;color:${L}">${step}</div><div style="font-size:17px;font-weight:600;letter-spacing:-0.01em;color:${G}">${heading}</div></div><div style="display:flex;flex-direction:column;gap:10px;padding:0 20px 18px">${inner}</div><div style="display:flex;justify-content:flex-end;gap:8px;padding:12px 20px;border-top:1px solid ${FR};background:#FAFBFA">${footer}</div></div>`;
const field = (lbl, val, w = "100%") => `<div style="display:flex;flex-direction:column;gap:4px;${F}"><div style="font-size:11px;font-weight:500;color:${L}">${lbl}</div><div style="width:${w};height:28px;display:flex;align-items:center;padding:0 8px;border-radius:6px;box-shadow:0 0 0 1px #D3D7D5;font-size:13px;color:${G};box-sizing:border-box">${val}</div></div>`;
const sheetA = sheet("Connect — choose type", "Connect an account", "Step 1 of 2",
  `<div style="display:flex;align-items:center;gap:8px;${F}font-size:12px;color:${G}">${A.tile("anth", 18)}<div style="font-weight:600">Anthropic</div><div style="color:${L}">· Claude subscription</div></div>`
  + choice({ titleT: "Pool my own subscription accounts", body: "Relay gives each new session the account with the most headroom; running sessions stay put. Provider terms neither clearly allow nor forbid this for one person's own accounts.", chip: "OPT-IN · TERMS UNRESOLVED", chipC: AM, selected: true })
  + choice({ titleT: "Observe a T3 instance", body: "The instance keeps its own login. Relay records usage and quota and never switches the account.", chip: "DOCUMENTED PATH", chipC: L })
  + `<div style="display:flex;gap:10px">${field("Label (shown everywhere)", "Personal")}${field("Identifier shown as", "d•••@gmail.com")}</div>`,
  btn("Cancel") + btn("Continue", true));
const check = (ok, t, sub = "") => `<div style="display:flex;gap:10px;${F}"><div style="width:16px;height:16px;flex-shrink:0;border-radius:8px;display:flex;align-items:center;justify-content:center;font-size:10px;font-weight:700;color:#FFFFFF;background:${ok === true ? EV : ok === false ? EM : "#B9BEBB"}">${ok === true ? "✓" : ok === false ? "!" : "·"}</div><div style="display:flex;flex-direction:column;gap:2px"><div style="font-size:13px;color:${G}">${t}</div>${sub ? `<div style="font-size:11px;line-height:16px;color:${L}">${sub}</div>` : ""}</div></div>`;
const code = (t) => `<div style="padding:10px 12px;border-radius:6px;background:#F4F5F4;font-family:'SF Mono',Menlo,monospace;font-size:11px;line-height:17px;color:${G};white-space:pre">${t}</div>`;
const sheetB = sheet("Connect — Codex setup", "Point a T3 Codex instance at Relay", "Step 2 of 2 · OpenAI · Observe · “Dev”",
  check(true, "Relay is listening on 127.0.0.1:8317")
  + check(false, "T3 instance “Codex dev” uses Managed setup", "Managed Codex ignores custom provider settings. In T3, switch this instance to Existing setup, then add the launch arguments below.")
  + code(`-c model_provider="relay"\n-c model_providers.relay.base_url="http://127.0.0.1:8317/v1"\n-c model_providers.relay.wire_api="responses"`)
  + check(null, "Waiting for the first request…", "Run any prompt in that T3 thread. Quota appears after the first response.")
  + `<div style="padding:10px 12px;border-radius:6px;background:color-mix(in srgb, ${AM} 9%, white);${F}font-size:11px;line-height:16px;color:#5E3F0C">Not yet tested live. With a custom provider, T3's own Codex account and plan display may not appear for this instance.</div>`,
  btn("Back") + btn("Copy arguments") + btn("Done", true));
const html04 = title("Relay · design review · settings 1 of 2", "Accounts & connect",
  "Accounts are grouped by provider, and each one carries a label you choose (Personal, Work, Secondary), a masked identifier, its plan and credential, its mode, and labelled state tags. Connecting asks for the label first, so the same name appears in the popover, menus and usage.")
  + `<div layer-name="Scenes" style="display:flex;gap:40px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:740px">${windowChrome("Settings — Accounts", "Accounts", accountsBody, 740)}${cap("States shown", "→ Next for new sessions, ● In use, ! Exhausted (with reset), ○ Disconnected (Reconnect), Ⅱ Paused (Resume), ◷ Stale (last seen). " + brandNote)}</div><div style="display:flex;flex-direction:column;gap:24px;width:460px">${sheetA}${sheetB}</div></div>`;

// ---------- 05 usage + routing ----------
const seg = (opts, active) => `<div style="display:flex;padding:2px;border-radius:6px;background:#EEF0EF">${opts.map((o) => `<div style="padding:3px 10px;border-radius:4px;${o === active ? "background:#FFFFFF;box-shadow:0 1px 2px rgba(0,0,0,0.1);" : ""}${F}font-size:11px;font-weight:500;color:${o === active ? G : L}">${o}</div>`).join("")}</div>`;
const NUM = ["Input", "Output", "Cached", "Req", "Errors"];
const uth = `<div style="display:flex;align-items:center;gap:8px;height:28px;padding:0 20px;border-bottom:1px solid ${FR};${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="width:168px;flex-shrink:0">Account</div>${NUM.map((t) => `<div style="width:56px;flex-shrink:0;text-align:right">${t}</div>`).join("")}<div style="flex:1;padding-left:8px">Share</div></div>`;
const urow = (p, label, plan, v, share, bold = false) => `<div style="display:flex;align-items:center;gap:8px;height:38px;padding:0 20px;border-bottom:1px solid ${FR};${F}font-size:12px;color:${G};${bold ? "font-weight:600;background:#FAFBFA;" : ""}"><div style="width:168px;flex-shrink:0;display:flex;align-items:center;gap:7px">${p ? A.tile(p, 14) : `<div style="width:14px"></div>`}<div style="display:flex;flex-direction:column"><div style="font-weight:600;font-size:12px;white-space:nowrap">${label}</div><div style="font-size:10px;color:${L};font-weight:400;white-space:nowrap">${plan}</div></div></div>${v.map((x) => `<div style="width:56px;flex-shrink:0;text-align:right">${x}</div>`).join("")}<div style="flex:1;padding-left:8px">${share == null ? "" : `<div style="height:4px;border-radius:3px;background:${FR};display:flex"><div style="width:${share}%;height:4px;border-radius:3px;background:${EV}"></div></div>`}</div></div>`;
const usageBody = `<div style="display:flex;flex-direction:column"><div style="display:flex;align-items:center;gap:10px;padding:16px 20px 12px;${F}"><div style="font-size:15px;font-weight:600;color:${G}">Usage</div><div style="font-size:12px;color:${L}">measured by Relay</div><div style="flex:1"></div>${seg(["24h", "7 days", "30 days"], "7 days")}</div>${uth}
${urow("anth", "Personal", "Claude Max", ["3.1M", "412K", "18.4M", "1,204", "3"], 38)}
${urow("anth", "Work", "Claude Max", ["2.2M", "288K", "11.0M", "806", "1"], 26)}
${urow("anth", "Secondary", "Claude Max", ["1.4M", "190K", "6.2M", "512", "14"], 16)}
${urow("anth", "Key A · Key B", "API keys", ["0.4M", "52K", "1.1M", "140", "0"], 5)}
${urow("oai", "Dev", "ChatGPT Plus · Codex", ["0.9M", "160K", "2.8M", "331", "0"], 11)}
${urow("oai", "Team", "ChatGPT Plus · Codex", ["0.3M", "41K", "0.9M", "97", "2"], 4)}
${urow(null, "Total", "summed, never averaged", ["8.3M", "1.14M", "40.4M", "3,090", "20"], null, true)}
<div style="padding:12px 20px 16px;${F}font-size:11px;line-height:16px;color:${L}">Counts only requests that passed through Relay, so these numbers will not match the provider's dashboard.</div></div>`;
const toggle = (on) => `<div style="width:30px;height:18px;flex-shrink:0;border-radius:9px;background:${on ? EV : "#D3D7D5"};display:flex;align-items:center;justify-content:${on ? "flex-end" : "flex-start"};padding:0 2px"><div style="width:14px;height:14px;border-radius:7px;background:#FFFFFF;box-shadow:0 1px 2px rgba(0,0,0,0.2)"></div></div>`;
const setRow = (label, sub, control) => `<div style="display:flex;align-items:center;gap:16px;padding:10px 0;border-bottom:1px solid ${FR};${F}"><div style="display:flex;flex-direction:column;gap:2px;flex:1"><div style="font-size:13px;color:${G}">${label}</div>${sub ? `<div style="font-size:11px;line-height:16px;color:${L}">${sub}</div>` : ""}</div>${control}</div>`;
const box = (t, w = 90) => `<div style="width:${w}px;height:26px;flex-shrink:0;display:flex;align-items:center;padding:0 8px;border-radius:5px;box-shadow:0 0 0 1px #D3D7D5;${F}font-size:12px;color:${G}">${t}</div>`;
const capRow = (p, prov, observe, pool) => `<div style="display:flex;align-items:center;gap:10px;height:32px;border-bottom:1px solid ${FR};${F}font-size:12px;color:${G}"><div style="flex:1;display:flex;align-items:center;gap:7px">${p ? A.tile(p, 14) : `<div style="width:14px"></div>`}${prov}</div><div style="width:150px;flex-shrink:0">${observe}</div><div style="width:170px;flex-shrink:0">${pool}</div></div>`;
const routingBody = `<div style="display:flex;flex-direction:column;padding:16px 20px 18px;gap:14px"><div style="display:flex;flex-direction:column">${sectionLabel("Pooled groups · new sessions")}
${setRow("Strategy", "Most headroom first; when two are close, use the one that resets sooner.", seg(["Quota-aware", "Round robin", "Priority"], "Quota-aware"))}
${setRow("Running sessions stay on their account", "Only new sessions are placed by headroom. A session moves only if its account is exhausted.", toggle(true))}
${setRow("Skip an account below", "", box("5% left", 80))}</div>
<div style="display:flex;flex-direction:column">${sectionLabel("Quota data")}
${setRow("Treat values as stale after", "Stale values show their age and are left out of averages.", box("15 min", 80))}
${setRow("Poll quota while idle", "Off: values update only when requests pass through.", toggle(false))}</div>
<div style="display:flex;flex-direction:column">${sectionLabel("Endpoint")}
${setRow("Local address", "", box("127.0.0.1:8317", 130))}
${setRow("Open at login", "", toggle(true))}</div>
<div style="display:flex;flex-direction:column;gap:4px">${sectionLabel("Capabilities in this build")}
<div style="display:flex;align-items:center;gap:10px;height:26px;${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="flex:1">Credential</div><div style="width:150px">Observe</div><div style="width:170px">Pool</div></div>
${capRow("anth", "Claude subscription", A.tag("DOCUMENTED", L), A.tag("OPT-IN · TERMS UNRESOLVED", AM))}
${capRow("anth", "Anthropic API key", A.tag("YES", L), A.tag("AVAILABLE", EV))}
${capRow("oai", "ChatGPT / Codex subscription", A.tag("T3 EXISTING · UNTESTED", L), A.tag("OPT-IN · TERMS UNRESOLVED", AM))}
${capRow("oai", "OpenAI API key", A.tag("YES", L), A.tag("AVAILABLE", EV))}
${capRow(null, "Other providers", A.tag("NOT EVALUATED", L), A.tag("NOT EVALUATED", L))}
<div style="padding-top:6px;${F}font-size:11px;line-height:16px;color:${L}">Statuses describe what this build does, not legal advice.</div></div></div>`;
const html05 = title("Relay · design review · settings 2 of 2", "Usage & routing",
  "Usage rows use the same provider tile and account label as the popover. Tokens are summed per account. Routing applies to pooled groups only; observed groups follow the T3 instance. Defaults: quota-aware placement of new sessions, running sessions sticky, idle polling off.")
  + `<div layer-name="Scenes" style="display:flex;gap:40px;align-items:flex-start">${windowChrome("Settings — Usage", "Usage", usageBody, 640)}${windowChrome("Settings — Routing", "Routing", routingBody, 560)}</div>`;

// ---------- 06 states ----------
const bar = (pct, c, opts = {}) => opts.hatch
  ? `<div style="width:120px;height:4px;flex-shrink:0;border-radius:3px;background-image:repeating-linear-gradient(135deg,#D5D9D7 0px,#D5D9D7 2px,transparent 2px,transparent 5px);background-color:${FR}"></div>`
  : `<div style="width:120px;height:4px;flex-shrink:0;border-radius:3px;background:${FR};display:flex"><div style="width:${pct}%;height:4px;border-radius:3px;background:${c};opacity:${opts.op ?? 1}"></div></div>`;
const slot = (inner) => `<div style="width:150px;flex-shrink:0;display:flex">${inner}</div>`;
const stateRow = (name, sample, value, rule) => `<div style="display:flex;align-items:center;gap:24px;padding:13px 0;border-bottom:1px solid ${FR};${F}"><div style="width:130px;flex-shrink:0;font-size:14px;font-weight:600;color:${G}">${name}</div>${sample}<div style="width:130px;flex-shrink:0;font-size:13px;font-weight:500;color:${G}">${value}</div><div style="flex:1;font-size:13px;line-height:19px;color:${L}">${rule}</div></div>`;
const html06 = title("Relay · design review · semantics", "States & numbers",
  "Every surface uses these rules. Bars always show quota <b>left</b>. Averages are per provider + plan + credential type + window, over fresh accounts only. Status is a glyph plus a word; colour only reinforces it.")
  + `<div layer-name="States" style="display:flex;flex-direction:column;max-width:1100px">
${stateRow("Next", slot(A.NEXT), "Personal", "Pooled group only: the account the next <b>new</b> session will use. Not the same as in use.")}
${stateRow("In use", slot(A.INUSE(2)), "Work", "Account currently carrying sessions. Running sessions stay on it while it has headroom.")}
${stateRow("Healthy", bar(72, EV), "72%", "Fresh value, 40% or more left.")}
${stateRow("Low", bar(22, AM), "22%", "Fresh value under 40% left. Pooled groups start preferring other accounts.")}
${stateRow("Exhausted", slot(A.EXH), `<span style="color:${EM}">0% · resets 38m</span>`, "Provider reported the limit is reached. Shows the reset time when reported, otherwise “reset unknown”. Pooled: no new sessions until reset.")}
${stateRow("Stale", slot(A.STALE), `<span style="color:${L}">58% · 18m ago</span>`, "Last value older than 15 min: dimmed bar with its age, left out of averages, counted as “n stale”.")}
${stateRow("Unknown", bar(0, L, { hatch: true }), `<span style="color:${L}">— not reported</span>`, "No value for this window yet. Never shown as 0%, never averaged.")}
${stateRow("Disconnected", slot(A.tag("○ DISCONNECTED", L)), `<span style="color:${L}">Reconnect</span>`, "T3 instance missing, auth rejected, or key revoked. No quota shown; one action to fix.")}
${stateRow("Paused", slot(A.tag("Ⅱ PAUSED", L)), `<span style="color:${L}">Resume</span>`, "Set by you. Quota still shown; never chosen by the pool.")}
${stateRow("Group average", bar(48, EV), "48% · avg · 3 fresh", "Mean over fresh accounts in one comparable group and window; the count is always printed. 5-hour and weekly windows are never combined. Tokens are summed, never averaged.")}
</div>`;

// ---------- 08 interaction, accessibility, motion ----------
const K = kit(LIGHT);
const stateHead = (lbl, style) => `<div style="display:flex;flex-direction:column;gap:6px;width:200px;${F}"><div style="font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}">${lbl}</div><div style="display:flex;align-items:center;gap:8px;height:28px;padding:0 8px;border-radius:6px;${style}"><svg width="10" height="10" viewBox="0 0 10 10"><path d="M3.5 2 L7 5 L3.5 8" fill="none" stroke="${L}" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg><div style="font-size:13px;font-weight:600;color:${G}">Claude Max</div><div style="font-size:11px;color:${L}">3</div></div></div>`;
const states = `<div style="display:flex;gap:20px;flex-wrap:wrap">${stateHead("Rest", "")}${stateHead("Hover", "background:#F2F4F3;")}${stateHead("Pressed (pointer-down)", "background:#E6E9E7;")}${stateHead("Keyboard focus", "box-shadow:0 0 0 2px #FFFFFF,0 0 0 4px #2F6FDB;")}${stateHead("Disabled (routing paused)", "opacity:0.45;")}</div>`;
const specTable = (rows, cols) => `<div style="display:flex;flex-direction:column">${rows.map((r, i) => `<div style="display:flex;gap:16px;padding:9px 0;border-bottom:1px solid ${FR};${F}font-size:12px;line-height:18px;color:${i === 0 ? L : G};${i === 0 ? "font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;" : ""}">${r.map((c, j) => `<div style="${cols[j] ? `width:${cols[j]}px;flex-shrink:0` : "flex:1"};${j > 0 && i > 0 ? `color:${L}` : ""}">${c}</div>`).join("")}</div>`).join("")}</div>`;
const section = (h, inner, w = 560) => `<div style="display:flex;flex-direction:column;gap:12px;width:${w}px;flex-shrink:0"><div style="${F}font-size:15px;font-weight:600;color:${G}">${h}</div>${inner}</div>`;
const motion = specTable([
  ["Interaction", "Motion", "Reduced motion"],
  ["Pointer-down on any control", "Pressed tint appears on the same frame; commit on release; drag-off cancels", "Same"],
  ["Popover opens", "Fades in and settles from 98% scale at the tray anchor; critically damped spring, damping 1.0, response 0.3 s; no bounce", "Opacity fade ≈ 120 ms, no scale"],
  ["Popover closes (Esc, outside click, icon)", "Reverse of open along the same path, from its current presentation", "Opacity fade"],
  ["Group expand / collapse", "Height and chevron on one spring, damping 1.0, response 0.35 s; starts from the current height, so it can be reversed mid-way", "Instant layout change; chevron swaps"],
  ["Value updates (≤ 2 Hz)", "Bar width eases to the new value; numbers change without counting animations", "Instant"],
], [190, null, 170]);
const keys = specTable([
  ["Key", "Action"],
  ["Click tray icon / global shortcut", "Open; focus moves to the first group header"],
  ["Tab / Shift-Tab", "Group headers → account cards (when expanded) → Pause → Settings → Open Relay; no trap, wraps inside the popover"],
  ["⌘O (Ctrl+O on Linux)", "Open Relay: show or focus the main window on Quotas, then close the popover"],
  ["↑ ↓", "Previous / next group or account"],
  ["Space / Return, → ←", "Toggle expand; → expands, ← collapses or moves to parent"],
  ["Esc, click outside, click icon again", "Close and return focus to the previously active app or window"],
  ["⌘, (Ctrl+, on Linux)", "Open Settings"],
], [220, null]);
const sr = specTable([
  ["Element", "Accessible name and value (example)"],
  ["Group header", "“Claude Max, 3 subscriptions, pooled. 5-hour window 48 percent left, average of 3 fresh accounts. Collapsed, button.”"],
  ["Account card", "“Work, d…@acme.co. In use by 2 sessions. 5-hour 64 percent left, resets in 3 hours. Weekly 70 percent left.”"],
  ["Stale value", "“Team, 5-hour 58 percent left, stale, last seen 18 minutes ago, not in average.”"],
  ["Unknown value", "“Weekly, not reported.” (never “0 percent”)"],
  ["Footer buttons", "“Open Relay, opens the main window on Quotas”; “Pause routing”; “Settings”. Icon buttons always carry names and tooltips."],
], [140, null]);
const targets = `<div style="display:flex;gap:28px;align-items:flex-start">${`<div style="display:flex;flex-direction:column;gap:6px;width:340px;${F}"><div style="position:relative;display:flex;flex-direction:column;gap:6px;padding:8px;border-radius:8px;box-shadow:0 0 0 1px ${FR}"><div style="display:flex;align-items:center;gap:8px;height:28px;padding:0 8px;border-radius:6px;background:color-mix(in srgb, #2F6FDB 10%, transparent);box-shadow:inset 0 0 0 1px color-mix(in srgb, #2F6FDB 40%, transparent)"><div style="font-size:13px;font-weight:600;color:${G}">Claude Max</div><div style="flex:1"></div><div style="font-size:10px;color:#2F6FDB">whole row · 28 pt</div></div><div style="display:flex;gap:6px">${["Ⅱ", "⚙", "Open Relay ↗"].map((t) => `<div style="display:flex;align-items:center;justify-content:center;min-width:28px;height:28px;padding:0 ${t.length > 2 ? 10 : 0}px;border-radius:6px;background:color-mix(in srgb, #2F6FDB 10%, transparent);box-shadow:inset 0 0 0 1px color-mix(in srgb, #2F6FDB 40%, transparent);font-size:12px;color:${G}">${t}</div>`).join("")}<div style="display:flex;align-items:center;font-size:10px;color:#2F6FDB">28 pt min</div></div></div><div style="font-size:11px;line-height:16px;color:${L}">Shaded areas are hit targets. Rows and buttons are at least 28 pt tall with 6 pt spacing; the whole header row toggles, not just the chevron.</div></div>`}<div style="display:flex;flex-direction:column;gap:8px;width:200px;${F}">${[["Translucent (macOS, default)", "rgba(255,255,255,0.86)", "If GPUI exposes a vibrancy window background; unverified"], ["Solid (Reduce Transparency, Linux)", "#FFFFFF", "Always available"], ["Increase Contrast", "#FFFFFF", "Adds a 1 pt #8A908D border, darker labels"]].map(([n, c, d]) => `<div style="display:flex;gap:8px;align-items:flex-start"><div style="width:28px;height:28px;flex-shrink:0;border-radius:6px;background:${c};box-shadow:0 0 0 1px ${n.startsWith("Increase") ? "#8A908D" : FR}, 0 2px 6px rgba(0,0,0,0.08)"></div><div style="display:flex;flex-direction:column;gap:1px"><div style="font-size:12px;font-weight:600;color:${G}">${n}</div><div style="font-size:11px;line-height:15px;color:${L}">${d}</div></div></div>`).join("")}</div></div>`;
const html08 = title("Relay · design review · behaviour spec", "Interaction, accessibility & motion",
  "Static boards cannot show motion; this sheet specifies it for the Rust/GPUI implementation. It follows Apple's fluid-interface guidance (instant feedback, interruptible critically damped springs, symmetric open and close) and adapts on Linux to the system font, solid surfaces, and compositor placement.")
  + `<div style="display:flex;flex-direction:column;gap:12px">${`<div style="${F}font-size:15px;font-weight:600;color:${G}">Control states</div>`}${states}</div>`
  + `<div style="display:flex;gap:48px;align-items:flex-start">${section("Motion", motion, 600)}${section("Keyboard & dismissal", keys, 520)}</div>`
  + `<div style="display:flex;gap:48px;align-items:flex-start">${section("Screen reader (VoiceOver; AT-SPI on Linux)", sr + `<div style="${F}font-size:11px;line-height:16px;color:${L}">GPUI accessibility-tree support must be verified before implementation; if missing, it is a blocker to resolve, not a reason to drop these semantics.</div>`, 600)}${section("Hit areas & materials", targets, 560)}</div>`
  + `<div style="${F}font-size:12px;line-height:18px;color:${L};max-width:900px">Typography: the platform system UI font (SF Pro on macOS; the desktop's UI font on Linux, such as Cantarell or Noto Sans), 11–15 pt with tabular numbers. No Apple fonts or SF Symbols are bundled or assumed on Linux; status glyphs are ordinary Unicode characters.</div>`;

// ---------- run ----------
await tools.paper.delete_nodes({ fileId, nodeIds: OLD });
const out = {};
const mk = async (key, name, html, w) => {
  out[key] = idOf(await tools.paper.create_artboard({ fileId, name, styles: board(w) }));
  return tools.paper.write_html({ fileId, targetNodeId: out[key], mode: "insert-children", html });
};
const w2 = await mk("a2", "02 · Tray popover — collapsed (light & dark)", html02, 1120);
const cloneId = idOf(w2, "Popover — light");
await mk("a3", "03 · Tray popover — expanded (macOS & Linux)", html03(cloneId), 1120);
await mk("a4", "04 · Settings — Accounts & connect", html04, 1340);
await mk("a5", "05 · Settings — Usage & routing", html05, 1340);
await mk("a6", "06 · States & numbers", html06, 1340);
await mk("a8", "08 · Interaction, accessibility & motion", html08, 1340);
return out;
