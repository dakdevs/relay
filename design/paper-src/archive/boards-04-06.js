// Executor `execute` program that creates Relay boards 04–06 (settings window + state legend) in Paper.
// Values are design examples only.
const fileId = "01M58Z151ZVWZ4VVK921R3Q5J9";
const F = `font-family:var(--font-ui);font-variant-numeric:tabular-nums;`;
const G = "#1C1F1E",
  L = "#6B716E",
  FR = "#ECEEED",
  EV = "#1F6B4F",
  AM = "#B7791F",
  EM = "#B4432F";
const idOf = (res) =>
  JSON.stringify(res).match(/\\"id\\": \\"([0-9A-Z]+-[0-9]+)\\"/)[1];
const boardStyles = {
  width: "1320px",
  height: "fit-content",
  background: "#FFFFFF",
  display: "flex",
  flexDirection: "column",
  gap: "32px",
  padding: "48px",
};
const title = (eyebrow, h, body) =>
  `<div layer-name="Board title" style="display:flex;flex-direction:column;gap:6px;${F}"><div style="font-size:11px;font-weight:500;letter-spacing:0.08em;text-transform:uppercase;color:${EV}">${eyebrow}</div><div style="font-size:30px;font-weight:700;letter-spacing:-0.02em;color:${G}">${h}</div><div style="font-size:14px;line-height:21px;color:${L};max-width:820px">${body}</div></div>`;
const cap = (h, b) =>
  `<div style="display:flex;flex-direction:column;gap:4px;${F}"><div style="font-size:13px;font-weight:600;color:${G}">${h}</div><div style="font-size:12px;line-height:18px;color:${L}">${b}</div></div>`;
const tag = (t, c) =>
  `<div style="flex-shrink:0;width:fit-content;align-self:flex-start;padding:1px 6px;border-radius:4px;background:color-mix(in srgb, ${c} 12%, transparent);${F}font-size:9px;font-weight:600;letter-spacing:0.07em;color:${c};white-space:nowrap">${t}</div>`;
const dot = (c, hollow = false) =>
  `<div style="width:7px;height:7px;flex-shrink:0;border-radius:4px;${hollow ? `border:1.5px solid ${c};width:5px;height:5px;` : `background:${c};`}"></div>`;
const btn = (t, primary = false) =>
  `<div style="display:flex;align-items:center;height:26px;padding:0 12px;border-radius:6px;${primary ? `background:${EV};color:#FFFFFF;` : `background:#FFFFFF;color:${G};box-shadow:0 0 0 0.5px rgba(0,0,0,0.2),0 1px 1px rgba(0,0,0,0.06);`}${F}font-size:12px;font-weight:500;white-space:nowrap">${t}</div>`;
const lights = `<div style="display:flex;gap:8px;width:60px">${["#FF5F57", "#FEBC2E", "#28C840"].map((c) => `<div style="width:12px;height:12px;border-radius:6px;background:${c}"></div>`).join("")}</div>`;
const tabs = (active) =>
  `<div style="display:flex;padding:2px;border-radius:7px;background:#E4E7E5">${["Accounts", "Usage", "Routing"].map((t) => `<div style="padding:3px 14px;border-radius:5px;${t === active ? "background:#FFFFFF;box-shadow:0 1px 2px rgba(0,0,0,0.12);" : ""}${F}font-size:12px;font-weight:${t === active ? 600 : 500};color:${t === active ? G : L}">${t}</div>`).join("")}</div>`;
const windowChrome = (name, active, body, w = 720) =>
  `<div layer-name="${name}" style="display:flex;flex-direction:column;width:${w}px;flex-shrink:0;background:#FFFFFF;border-radius:10px;overflow:hidden;box-shadow:0 18px 44px rgba(28,31,30,0.18),0 0 0 0.5px rgba(28,31,30,0.2)"><div style="display:flex;align-items:center;height:44px;padding:0 14px;background:#F4F5F4;border-bottom:1px solid ${FR}">${lights}<div style="flex:1;display:flex;justify-content:center">${tabs(active)}</div><div style="width:60px"></div></div>${body}</div>`;
const sectionLabel = (t) =>
  `<div style="${F}font-size:10px;font-weight:600;letter-spacing:0.08em;text-transform:uppercase;color:${L}">${t}</div>`;

// ---------- board 04: accounts + connect ----------
const th = (cells) =>
  `<div style="display:flex;align-items:center;gap:12px;height:28px;padding:0 20px;border-bottom:1px solid ${FR};${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}">${cells.map(([t, w]) => `<div style="${w ? `width:${w}px;flex-shrink:0` : "flex:1"}">${t}</div>`).join("")}</div>`;
const acctRow = ({
  name,
  prov,
  mode,
  status,
  sColor,
  hollow = false,
  action = "",
  dim = false,
}) =>
  `<div style="display:flex;align-items:center;gap:12px;height:40px;padding:0 20px;border-bottom:1px solid ${FR};${F}font-size:13px;color:${G};${dim ? "opacity:0.6;" : ""}"><div style="width:110px;flex-shrink:0;font-weight:500">${name}</div><div style="width:130px;flex-shrink:0;color:${L}">${prov}</div><div style="width:80px;flex-shrink:0">${mode === "pooled" ? tag("POOLED", EV) : tag("OBSERVED", L)}</div><div style="flex:1;display:flex;align-items:center;gap:7px;font-size:12px;color:${sColor === G ? L : sColor}">${dot(sColor, hollow)}<div style="white-space:nowrap">${status}</div></div><div style="width:92px;flex-shrink:0;display:flex;justify-content:flex-end">${action}</div></div>`;
const accountsBody = `<div style="display:flex;flex-direction:column"><div style="display:flex;align-items:center;gap:10px;padding:16px 20px 12px;${F}"><div style="font-size:15px;font-weight:600;color:${G}">Accounts</div><div style="font-size:12px;color:${L}">8 · 3 groups</div><div style="flex:1"></div>${btn("Connect…", true)}</div>
${th([["Account", 110], ["Provider · plan", 130], ["Mode", 80], ["Status"], ["", 92]])}
${acctRow({ name: "personal", prov: "Claude Max", mode: "pooled", status: "Healthy · next for new sessions", sColor: EV })}
${acctRow({ name: "work", prov: "Claude Max", mode: "pooled", status: "Healthy · seen 4m ago", sColor: EV })}
${acctRow({ name: "archive", prov: "Claude Max", mode: "pooled", status: "Exhausted · 5-hour resets in 38m", sColor: EM })}
${acctRow({ name: "old-work", prov: "Claude Pro", mode: "observed", status: "Disconnected · T3 instance not found", sColor: L, hollow: true, action: btn("Reconnect") })}
${acctRow({ name: "dev", prov: "Codex Plus", mode: "observed", status: "Healthy · seen 3m ago", sColor: EV })}
${acctRow({ name: "team", prov: "Codex Plus", mode: "observed", status: "Stale · last seen 18m ago", sColor: AM })}
${acctRow({ name: "key-a", prov: "Anthropic API", mode: "pooled", status: "Healthy · seen 20s ago", sColor: EV })}
${acctRow({ name: "key-b", prov: "Anthropic API", mode: "pooled", status: "Paused by you · not routed", sColor: L, action: btn("Resume"), dim: false })}
<div style="padding:12px 20px 16px;${F}font-size:11px;line-height:16px;color:${L}">Relay never reads or changes T3's saved logins. Observed accounts are matched to T3 instances by their Relay endpoint; usage counts only traffic that passes through Relay.</div></div>`;
const choice = ({
  titleT,
  body,
  chip,
  chipC,
  selected = false,
  disabled = false,
}) =>
  `<div style="display:flex;gap:12px;padding:12px 14px;border-radius:8px;${selected ? `box-shadow:0 0 0 1.5px ${EV};background:color-mix(in srgb, ${EV} 4%, white);` : `box-shadow:0 0 0 1px ${FR};`}${disabled ? "opacity:0.62;" : ""}"><div style="width:14px;height:14px;flex-shrink:0;margin-top:2px;border-radius:7px;${selected ? `border:4px solid ${EV};width:6px;height:6px;` : `border:1.5px solid #B9BEBB;width:11px;height:11px;`}"></div><div style="display:flex;flex-direction:column;gap:4px;flex:1;${F}"><div style="display:flex;align-items:center;gap:8px"><div style="font-size:13px;font-weight:600;color:${G}">${titleT}</div>${tag(chip, chipC)}</div><div style="font-size:12px;line-height:17px;color:${L}">${body}</div></div></div>`;
const sheet = (name, heading, step, inner, footer) =>
  `<div layer-name="${name}" style="display:flex;flex-direction:column;width:460px;flex-shrink:0;background:#FFFFFF;border-radius:12px;overflow:hidden;box-shadow:0 22px 50px rgba(28,31,30,0.22),0 0 0 0.5px rgba(28,31,30,0.2)"><div style="display:flex;flex-direction:column;gap:3px;padding:18px 20px 14px;${F}"><div style="font-size:11px;color:${L}">${step}</div><div style="font-size:17px;font-weight:600;letter-spacing:-0.01em;color:${G}">${heading}</div></div><div style="display:flex;flex-direction:column;gap:10px;padding:0 20px 18px">${inner}</div><div style="display:flex;justify-content:flex-end;gap:8px;padding:12px 20px;border-top:1px solid ${FR};background:#FAFBFA">${footer}</div></div>`;
const sheetA = sheet(
  "Connect — choose type",
  "Connect an account",
  "Step 1 of 2",
  choice({
    titleT: "Pool my own subscription accounts",
    body: "Relay gives each new session the account with the most headroom; running conversations stay put. Provider terms neither clearly allow nor forbid this for one person's own accounts.",
    chip: "OPT-IN · TERMS UNRESOLVED",
    chipC: AM,
    selected: true,
  }) +
    choice({
      titleT: "Observe a T3 instance",
      body: "The T3 instance keeps its own login. Relay records usage and quota and never switches the account.",
      chip: "DOCUMENTED PATH",
      chipC: L,
    }) +
    choice({
      titleT: "Add an API key",
      body: "Anthropic Console or OpenAI Platform key, pooled by headroom.",
      chip: "POOLABLE",
      chipC: EV,
    }),
  btn("Cancel") + btn("Continue", true),
);
const check = (ok, t, sub = "") =>
  `<div style="display:flex;gap:10px;${F}"><div style="width:16px;height:16px;flex-shrink:0;border-radius:8px;display:flex;align-items:center;justify-content:center;font-size:10px;font-weight:700;color:#FFFFFF;background:${ok === true ? EV : ok === false ? EM : "#B9BEBB"}">${ok === true ? "✓" : ok === false ? "!" : "·"}</div><div style="display:flex;flex-direction:column;gap:2px"><div style="font-size:13px;color:${G}">${t}</div>${sub ? `<div style="font-size:11px;line-height:16px;color:${L}">${sub}</div>` : ""}</div></div>`;
const code = (t) =>
  `<div style="padding:10px 12px;border-radius:6px;background:#F4F5F4;font-family:'SF Mono',Menlo,monospace;font-size:11px;line-height:17px;color:${G};white-space:pre">${t}</div>`;
const sheetB = sheet(
  "Connect — Codex setup",
  "Point a T3 Codex instance at Relay",
  "Step 2 of 2 · Observe · Codex",
  check(true, "Relay is listening on 127.0.0.1:8317") +
    check(
      false,
      "T3 instance “Codex dev” uses Managed setup",
      "Managed Codex ignores custom provider settings. In T3, switch this instance to Existing setup, then add the launch arguments below.",
    ) +
    code(
      `-c model_provider="relay"\n-c model_providers.relay.base_url="http://127.0.0.1:8317/v1"\n-c model_providers.relay.wire_api="responses"`,
    ) +
    check(
      null,
      "Waiting for the first request…",
      "Run any prompt in that T3 thread. Relay matches the instance when its traffic arrives; quota appears after the first response.",
    ) +
    `<div style="padding:10px 12px;border-radius:6px;background:color-mix(in srgb, ${AM} 9%, white);${F}font-size:11px;line-height:16px;color:#6E4A12">Not yet tested live. With a custom provider, T3's own Codex account and plan display may not appear for this instance.</div>`,
  btn("Back") + btn("Copy arguments") + btn("Done", true),
);
const claudeNote = `<div layer-name="Claude note" style="display:flex;flex-direction:column;gap:8px;width:460px;padding:14px 16px;border-radius:10px;box-shadow:0 0 0 1px ${FR};${F}"><div style="font-size:13px;font-weight:600;color:${G}">Claude variant of step 2</div><div style="font-size:12px;line-height:18px;color:${L}">Set <span style="font-family:'SF Mono',Menlo,monospace;font-size:11px;color:${G}">ANTHROPIC_BASE_URL</span> only, so the instance keeps its claude.ai login (documented pass-through). Relay warns before a router key is used instead: that replaces the subscription login, and T3 stops showing subscription limits for the instance.</div></div>`;
const html04 =
  title(
    "Relay · design review · settings 1 of 2",
    "Accounts & connect",
    "One list for every account, with mode and status side by side. Connecting asks how the account should be used: pooled (Relay picks one for each new session) or observed (the T3 instance keeps choosing). Pooling your own subscriptions is opt-in, and its terms status is shown, not decided.",
  ) +
  `<div layer-name="Scenes" style="display:flex;gap:40px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px;width:720px">${windowChrome("Settings — Accounts", "Accounts", accountsBody)}${cap("States shown", "Healthy, exhausted (with reset), disconnected (T3 instance gone → Reconnect), stale (last seen time), paused by the user (not routed, still observed).")}</div><div style="display:flex;flex-direction:column;gap:24px;width:460px">${sheetA}${sheetB}${claudeNote}</div></div>`;

// ---------- board 05: usage + routing ----------
const seg = (opts, active) =>
  `<div style="display:flex;padding:2px;border-radius:6px;background:#EEF0EF">${opts.map((o) => `<div style="padding:2px 10px;border-radius:4px;${o === active ? "background:#FFFFFF;box-shadow:0 1px 2px rgba(0,0,0,0.1);" : ""}${F}font-size:11px;font-weight:500;color:${o === active ? G : L}">${o}</div>`).join("")}</div>`;
const uth = `<div style="display:flex;align-items:center;gap:10px;height:28px;padding:0 20px;border-bottom:1px solid ${FR};${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="width:120px;flex-shrink:0">Account</div>${["Input", "Output", "Cached", "Requests", "Errors"].map((t) => `<div style="width:66px;flex-shrink:0;text-align:right">${t}</div>`).join("")}<div style="width:90px;flex-shrink:0">Share</div></div>`;
const urow = (name, grp, v, share, bold = false) =>
  `<div style="display:flex;align-items:center;gap:10px;height:36px;padding:0 20px;border-bottom:1px solid ${FR};${F}font-size:12px;color:${G};${bold ? "font-weight:600;background:#FAFBFA;" : ""}"><div style="width:120px;flex-shrink:0;display:flex;gap:6px;align-items:baseline;white-space:nowrap"><div style="font-weight:${bold ? 600 : 500};font-size:13px">${name}</div><div style="font-size:11px;color:${L};font-weight:400">${grp}</div></div>${v.map((x) => `<div style="width:66px;flex-shrink:0;text-align:right">${x}</div>`).join("")}<div style="width:90px;flex-shrink:0">${share == null ? "" : `<div style="height:4px;border-radius:3px;background:${FR};display:flex"><div style="width:${share}%;height:4px;border-radius:3px;background:${EV}"></div></div>`}</div></div>`;
const usageBody = `<div style="display:flex;flex-direction:column"><div style="display:flex;align-items:center;gap:10px;padding:16px 20px 12px;${F}"><div style="font-size:15px;font-weight:600;color:${G}">Usage</div><div style="font-size:12px;color:${L}">measured by Relay</div><div style="flex:1"></div>${seg(["24h", "7 days", "30 days"], "7 days")}</div>${uth}
${urow("personal", "Claude Max", ["3.1M", "412K", "18.4M", "1,204", "3"], 38)}
${urow("work", "Claude Max", ["2.2M", "288K", "11.0M", "806", "1"], 26)}
${urow("archive", "Claude Max", ["1.4M", "190K", "6.2M", "512", "14"], 16)}
${urow("dev", "Codex Plus", ["0.9M", "160K", "2.8M", "331", "0"], 11)}
${urow("team", "Codex Plus", ["0.3M", "41K", "0.9M", "97", "2"], 4)}
${urow("key-a, b", "Anthropic API", ["0.4M", "52K", "1.1M", "140", "0"], 5)}
${urow("Total", "summed", ["8.3M", "1.14M", "40.4M", "3,090", "20"], null, true)}
<div style="padding:12px 20px 16px;${F}font-size:11px;line-height:16px;color:${L}">Counts only requests that passed through Relay. Usage from other apps or devices on the same subscription is not included, so these numbers will not match the provider's dashboard.</div></div>`;
const toggle = (on) =>
  `<div style="width:30px;height:18px;flex-shrink:0;border-radius:9px;background:${on ? EV : "#D3D7D5"};display:flex;align-items:center;justify-content:${on ? "flex-end" : "flex-start"};padding:0 2px"><div style="width:14px;height:14px;border-radius:7px;background:#FFFFFF;box-shadow:0 1px 2px rgba(0,0,0,0.2)"></div></div>`;
const setRow = (label, sub, control) =>
  `<div style="display:flex;align-items:center;gap:16px;padding:10px 0;border-bottom:1px solid ${FR};${F}"><div style="display:flex;flex-direction:column;gap:2px;flex:1"><div style="font-size:13px;color:${G}">${label}</div>${sub ? `<div style="font-size:11px;line-height:16px;color:${L}">${sub}</div>` : ""}</div>${control}</div>`;
const field = (t, w = 90) =>
  `<div style="width:${w}px;height:24px;flex-shrink:0;display:flex;align-items:center;padding:0 8px;border-radius:5px;box-shadow:0 0 0 1px #D3D7D5;${F}font-size:12px;color:${G}">${t}</div>`;
const capRow = (prov, observe, pool) =>
  `<div style="display:flex;align-items:center;gap:10px;height:32px;border-bottom:1px solid ${FR};${F}font-size:12px;color:${G}"><div style="flex:1">${prov}</div><div style="width:150px;flex-shrink:0">${observe}</div><div style="width:170px;flex-shrink:0">${pool}</div></div>`;
const routingBody = `<div style="display:flex;flex-direction:column;padding:16px 20px 18px;gap:14px"><div style="display:flex;flex-direction:column">${sectionLabel("Pooled groups · new sessions")}
${setRow("Strategy", "Most headroom first; when two are close, use the one that resets sooner.", seg(["Quota-aware", "Round robin", "Priority"], "Quota-aware"))}
${setRow("Existing conversations stay on their account", "Only new sessions are placed by headroom. A conversation moves only if its account is exhausted.", toggle(true))}
${setRow("Skip an account below", "", field("5% left", 80))}</div>
<div style="display:flex;flex-direction:column">${sectionLabel("Quota data")}
${setRow("Treat values as stale after", "Stale values are shown with their age and left out of averages.", field("15 min", 80))}
${setRow("Poll quota while idle", "Off: values update only when requests pass through. On: Relay asks each account's usage endpoint periodically.", toggle(false))}</div>
<div style="display:flex;flex-direction:column">${sectionLabel("Endpoint")}
${setRow("Local address", "", field("127.0.0.1:8317", 130))}
${setRow("Relay key for T3 instances", "Used by pooled groups only.", `<div style="display:flex;gap:6px">${field("••••••••••••", 110)}${btn("Copy")}</div>`)}
${setRow("Open at login", "", toggle(true))}</div>
<div style="display:flex;flex-direction:column;gap:4px">${sectionLabel("Capabilities in this build")}
<div style="display:flex;align-items:center;gap:10px;height:26px;${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="flex:1">Credential</div><div style="width:150px">Observe</div><div style="width:170px">Pool</div></div>
${capRow("Claude subscription", tag("DOCUMENTED", L), tag("OPT-IN · TERMS UNRESOLVED", AM))}
${capRow("Codex subscription", tag("T3 EXISTING · UNTESTED", L), tag("OPT-IN · TERMS UNRESOLVED", AM))}
${capRow("Anthropic API key", tag("YES", L), tag("AVAILABLE", EV))}
${capRow("OpenAI API key", tag("YES", L), tag("AVAILABLE", EV))}
${capRow("Other providers", tag("NOT EVALUATED", L), tag("NOT EVALUATED", L))}
<div style="padding-top:6px;${F}font-size:11px;line-height:16px;color:${L}">Statuses describe what this build does, not legal advice. Review the providers' terms for your own use.</div></div></div>`;
const html05 =
  title(
    "Relay · design review · settings 2 of 2",
    "Usage & routing",
    "Usage is summed per account and labelled as measured locally. Routing settings apply to pooled groups only; observed groups follow the T3 instance. Defaults: quota-aware placement of new sessions, existing sessions sticky, idle polling off.",
  ) +
  `<div layer-name="Scenes" style="display:flex;gap:40px;align-items:flex-start">${windowChrome("Settings — Usage", "Usage", usageBody, 640)}${windowChrome("Settings — Routing", "Routing", routingBody, 560)}</div>`;

// ---------- board 06: states legend ----------
const bar = (pct, c, opts = {}) =>
  opts.hatch
    ? `<div style="width:120px;height:4px;flex-shrink:0;border-radius:3px;background-image:repeating-linear-gradient(135deg,#D5D9D7 0px,#D5D9D7 2px,transparent 2px,transparent 5px);background-color:${FR}"></div>`
    : `<div style="width:120px;height:4px;flex-shrink:0;border-radius:3px;background:${FR};display:flex"><div style="width:${pct}%;height:4px;border-radius:3px;background:${c};opacity:${opts.op ?? 1}"></div></div>`;
const stateRow = (name, sample, value, rule) =>
  `<div style="display:flex;align-items:center;gap:24px;padding:14px 0;border-bottom:1px solid ${FR};${F}"><div style="width:150px;flex-shrink:0;font-size:14px;font-weight:600;color:${G}">${name}</div>${sample}<div style="width:110px;flex-shrink:0;font-size:13px;font-weight:500;color:${G}">${value}</div><div style="flex:1;font-size:13px;line-height:19px;color:${L}">${rule}</div></div>`;
const html06 =
  title(
    "Relay · design review · semantics",
    "States & numbers",
    "Every surface uses these rules. Bars always show quota <b>left</b>. Averages are per provider + plan + credential type + window, over fresh accounts only.",
  ) +
  `<div layer-name="States" style="display:flex;flex-direction:column;max-width:1100px">
${stateRow("Healthy", bar(72, EV), "72%", "Fresh value, 40% or more left.")}
${stateRow("Low", bar(22, AM), "22%", "Fresh value under 40% left. Pooled groups start preferring other accounts.")}
${stateRow("Exhausted", bar(0, EM), `<span style="color:${EM}">0% · resets 38m</span>`, "Provider reported the limit is reached. Shows reset time when reported, otherwise “reset unknown”. Pooled: skipped until reset.")}
${stateRow("Stale", bar(58, EV, { op: 0.35 }), `<span style="color:${L}">58% · 18m ago</span>`, "Last value older than 15 min. Shown dimmed with its age; excluded from averages and counted as “n stale”.")}
${stateRow("Unknown", bar(0, L, { hatch: true }), `<span style="color:${L}">—</span>`, "No value reported for this window yet. Never shown as 0% and never averaged.")}
${stateRow("Window reset", bar(100, EV), "100%", "Reset time has passed with no new response: treated as full, marked “reset, not yet confirmed”.")}
${stateRow("Disconnected", `<div style="width:120px;flex-shrink:0;display:flex;align-items:center;gap:6px">${dot(L, true)}<div style="${F}font-size:12px;color:${L}">no quota</div></div>`, `<span style="color:${L}">Reconnect</span>`, "T3 instance missing, auth rejected, or key revoked. No quota shown; one action to fix.")}
${stateRow("Paused", `<div style="width:120px;flex-shrink:0;display:flex;align-items:center;gap:6px">${dot(L)}<div style="${F}font-size:12px;color:${L}">paused</div></div>`, `<span style="color:${L}">Resume</span>`, "Set by the user. Quota still shown; never chosen by the pool.")}
${stateRow("Group average", bar(48, EV), "48% · avg · 3 fresh", "Mean over fresh accounts in one comparable group and window. The count is always printed. Usage tokens are summed, never averaged.")}
</div>`;

// ---------- run ----------
const out = {};
for (const [key, name, html] of [
  ["a4", "04 · Settings — Accounts & connect", html04],
  ["a5", "05 · Settings — Usage & routing", html05],
  ["a6", "06 · States & numbers", html06],
]) {
  out[key] = idOf(
    await tools.paper.create_artboard({ fileId, name, styles: boardStyles }),
  );
  await tools.paper.write_html({
    fileId,
    targetNodeId: out[key],
    mode: "insert-children",
    html,
  });
}
return out;
