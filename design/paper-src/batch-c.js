// Batch C (run after kit.js): final boards 4–8 on the "Final Designs" page.
// Borderless tonal surfaces, official marks, no predicted-account indicator, tactile buttons. Design samples only.
const PAGE = "p-2-0";
const T = LT,
  K = Lk;
const W = {
  canvas: "#F1F2F1",
  chrome: "#E9EBEA",
  ctrl: "#E4E7E5",
  inset: "#F5F6F5",
};
const lights = `<div style="display:flex;gap:8px;width:60px">${["#FF5F57", "#FEBC2E", "#28C840"].map((c) => `<div style="width:12px;height:12px;border-radius:6px;background:${c}"></div>`).join("")}</div>`;
const tabs = (active) =>
  `<div style="display:flex;padding:2px;border-radius:8px;background:${W.ctrl}">${["Quotas", "Accounts", "Usage", "Routing"].map((t) => `<div style="padding:4px 14px;border-radius:6px;${t === active ? `background:#FFFFFF;` : ""}${F}font-size:12px;font-weight:${t === active ? 600 : 500};color:${t === active ? G : L}">${t}</div>`).join("")}</div>`;
const mainWin = (name, active, body, w = 1240, right = "") =>
  `<div layer-name="${name}" style="display:flex;flex-direction:column;width:${w}px;flex-shrink:0;background:${W.canvas};border-radius:12px;overflow:hidden;box-shadow:0 22px 50px rgba(28,31,30,0.16)"><div style="display:flex;align-items:center;height:46px;padding:0 14px;background:${W.chrome}">${lights}<div style="flex:1;display:flex;justify-content:center">${tabs(active)}</div><div style="width:60px;display:flex;justify-content:flex-end">${right}</div></div>${body}</div>`;
const surf = (inner, pad = "14px 16px", bg = "#FFFFFF") =>
  `<div style="display:flex;flex-direction:column;gap:10px;padding:${pad};border-radius:14px;background:${bg}">${inner}</div>`;
const label = (t) =>
  `<div style="${F}font-size:10px;font-weight:600;letter-spacing:0.08em;text-transform:uppercase;color:${L}">${t}</div>`;
const provHead = (mark, name, sub) =>
  `<div style="display:flex;align-items:center;gap:8px;padding:2px 4px;${F}">${box(mark, 18)}<div style="font-size:14px;font-weight:600;color:${G}">${name}</div><div style="font-size:12px;color:${L}">${sub}</div></div>`;
const groupHead = (mark, name, sub, mode) =>
  `<div style="display:flex;align-items:center;gap:8px;padding:6px 12px 2px;${F}">${mark ? box(mark, 16) : `<div style="width:16px"></div>`}<div style="font-size:13px;font-weight:600;color:${G}">${name}</div><div style="font-size:12px;color:${L}">${sub}</div>${mode}</div>`;
const POOLED = K.tag("POOLED", T.ev),
  OBSERVED = K.tag("OBSERVED", T.mu);
const DISC = K.tag("○ DISCONNECTED", T.mu),
  PAUSED = K.tag("Ⅱ PAUSED", T.mu);
const sm = (t, o = {}) => K.button(t, { h: 26, ...o });

// ---------- 4 Accounts & connect ----------
const aRow = (
  { label: lb, ident, plan, tags = "", status, sColor = L, action = "" },
  shade,
) =>
  `<div style="display:flex;align-items:center;gap:14px;min-height:44px;padding:6px 12px;border-radius:9px;${shade ? `background:${W.inset};` : ""}${F}"><div style="width:220px;flex-shrink:0;display:flex;flex-direction:column;gap:1px;padding-left:24px"><div style="font-size:13px;font-weight:600;color:${G}">${lb}</div><div style="font-size:11px;color:${L}">${ident}</div></div><div style="width:170px;flex-shrink:0;font-size:12px;color:${L}">${plan}</div><div style="width:150px;flex-shrink:0;display:flex">${tags}</div><div style="flex:1;font-size:12px;color:${sColor}">${status}</div><div style="width:110px;flex-shrink:0;display:flex;justify-content:flex-end">${action}</div></div>`;
const aCols = `<div style="display:flex;gap:14px;padding:4px 12px;${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="width:220px;padding-left:24px">Account</div><div style="width:170px">Plan · credential</div><div style="width:150px">State</div><div style="flex:1">Detail</div><div style="width:110px"></div></div>`;
const rows = (list) => list.map((r, i) => aRow(r, i % 2 === 0)).join("");
const accountsBody = `<div style="display:flex;flex-direction:column;gap:12px;padding:16px 20px 20px"><div style="display:flex;align-items:center;gap:10px;padding:0 4px;${F}"><div style="font-size:17px;font-weight:600;color:${G}">Accounts</div><div style="font-size:12px;color:${L}">8 accounts · 2 providers</div><div style="flex:1"></div>${K.button("Connect…", { kind: "primary" })}</div>
${surf(
  provHead(ANTH(12, T.anth), "Anthropic", "6 accounts") +
    aCols +
    groupHead(CLAUDE(14), "Claude Max", "3 subscriptions", POOLED) +
    rows([
      {
        label: "Personal",
        ident: "d•••@gmail.com",
        plan: "Max 20× · subscription",
        status: "Healthy · seen 2m ago",
      },
      {
        label: "Work",
        ident: "d•••@acme.co",
        plan: "Max 20× · subscription",
        tags: K.INUSE(2),
        status: "Healthy · seen 4m ago",
      },
      {
        label: "Secondary",
        ident: "d•••@proton.me",
        plan: "Max 20× · subscription",
        tags: K.EXH,
        status: "5-hour resets in 38m · Oct 18, 22:19",
        sColor: T.em,
      },
    ]) +
    groupHead(CLAUDE(14), "Claude Pro", "1 subscription", OBSERVED) +
    rows([
      {
        label: "Old work",
        ident: "d•••@oldco.io",
        plan: "Pro · subscription",
        tags: DISC,
        status: "T3 instance not found",
        action: sm("Reconnect"),
      },
    ]) +
    groupHead("", "API keys", "2 keys", POOLED) +
    rows([
      {
        label: "Key A",
        ident: "sk-ant-…7f2c",
        plan: "API key",
        status: "Healthy · seen 20s ago",
      },
      {
        label: "Key B",
        ident: "sk-ant-…91ab",
        plan: "API key",
        tags: PAUSED,
        status: "Paused by you · not routed",
        action: sm("Resume"),
      },
    ]),
  "14px 12px",
)}
${surf(
  provHead(OPENAI(15, T.gpt), "OpenAI", "2 accounts") +
    groupHead(
      CHATGPT(14, T.gpt),
      "ChatGPT Plus · Codex",
      "2 subscriptions",
      OBSERVED,
    ) +
    rows([
      {
        label: "Dev",
        ident: "d•••@acme.co",
        plan: "Plus · subscription",
        tags: K.INUSE(1),
        status: "Healthy · seen 3m ago",
      },
      {
        label: "Team",
        ident: "t•••@acme.co",
        plan: "Plus · subscription",
        tags: K.STALE,
        status: "Last seen 18m ago · not in averages",
        sColor: T.am,
      },
    ]),
  "14px 12px",
)}
<div style="padding:0 4px;${F}font-size:11px;line-height:16px;color:${L}">Labels are yours and stay fixed; identifiers are masked. Relay never reads or changes T3's saved logins and counts only traffic that passes through it.</div></div>`;
const field = (lb, val) =>
  `<div style="display:flex;flex-direction:column;gap:4px;flex:1;${F}"><div style="font-size:11px;font-weight:500;color:${L}">${lb}</div><div style="height:30px;display:flex;align-items:center;padding:0 10px;border-radius:7px;background:${W.inset};font-size:13px;color:${G}">${val}</div></div>`;
const radio = (on) =>
  `<div style="width:16px;height:16px;flex-shrink:0;margin-top:1px;border-radius:8px;background:${on ? T.ev : W.ctrl};display:flex;align-items:center;justify-content:center">${on ? `<div style="width:6px;height:6px;border-radius:3px;background:#FFFFFF"></div>` : ""}</div>`;
const choice = ({ t, body, chip, chipC, on = false }) =>
  `<div style="display:flex;gap:12px;padding:12px 14px;border-radius:10px;background:${on ? `color-mix(in srgb, ${T.ev} 10%, ${W.inset})` : W.inset}">${radio(on)}<div style="display:flex;flex-direction:column;gap:4px;flex:1;${F}"><div style="display:flex;align-items:center;gap:8px;flex-wrap:wrap"><div style="font-size:13px;font-weight:600;color:${G}">${t}</div>${K.tag(chip, chipC)}</div><div style="font-size:12px;line-height:17px;color:${L}">${body}</div></div></div>`;
const provPick = (mark, name, on) =>
  `<div style="display:flex;align-items:center;gap:8px;flex:1;height:40px;padding:0 12px;border-radius:9px;background:${on ? `color-mix(in srgb, ${T.ev} 10%, ${W.inset})` : W.inset};${F}font-size:13px;font-weight:${on ? 600 : 500};color:${G}">${box(mark, 18)}${name}</div>`;
const sheet = (name, step, heading, inner, foot) =>
  `<div layer-name="${name}" style="display:flex;flex-direction:column;width:470px;flex-shrink:0;background:#FFFFFF;border-radius:14px;overflow:hidden;box-shadow:0 22px 50px rgba(28,31,30,0.20)"><div style="display:flex;flex-direction:column;gap:3px;padding:18px 20px 12px;${F}"><div style="font-size:11px;color:${L}">${step}</div><div style="font-size:17px;font-weight:600;letter-spacing:-0.01em;color:${G}">${heading}</div></div><div style="display:flex;flex-direction:column;gap:10px;padding:0 20px 18px">${inner}</div><div style="display:flex;justify-content:flex-end;gap:8px;padding:12px 20px;background:${W.inset}">${foot}</div></div>`;
const check = (ok, t, sub = "") =>
  `<div style="display:flex;gap:10px;${F}"><div style="width:18px;height:18px;flex-shrink:0;border-radius:9px;display:flex;align-items:center;justify-content:center;font-size:10px;font-weight:700;color:#FFFFFF;background:${ok === true ? T.ev : ok === false ? T.em : "#B9BEBB"}">${ok === true ? "✓" : ok === false ? "!" : "·"}</div><div style="display:flex;flex-direction:column;gap:2px;flex:1"><div style="font-size:13px;color:${G}">${t}</div>${sub ? `<div style="font-size:11px;line-height:16px;color:${L}">${sub}</div>` : ""}</div></div>`;
const code = (t) =>
  `<div style="padding:10px 12px;border-radius:8px;background:${W.inset};font-family:'SF Mono',Menlo,monospace;font-size:11px;line-height:17px;color:${G};white-space:pre">${t}</div>`;
const sheetA = sheet(
  "Connect — choose provider and mode",
  "Step 1 of 2",
  "Connect an account",
  `<div style="display:flex;gap:8px">${provPick(ANTH(12, T.anth), "Anthropic", true)}${provPick(OPENAI(16, T.gpt), "OpenAI", false)}</div>` +
    `<div style="display:flex;align-items:center;gap:8px;${F}font-size:12px;color:${L}">${box(CLAUDE(14), 16)}Claude subscription · Max or Pro</div>` +
    choice({
      t: "Pool my own subscription accounts",
      body: "Relay gives each new session the account with the most headroom; running sessions stay on their account. Provider terms neither clearly allow nor forbid this for one person's own accounts.",
      chip: "OPT-IN · TERMS UNRESOLVED",
      chipC: T.am,
      on: true,
    }) +
    choice({
      t: "Observe a T3 instance",
      body: "The instance keeps its own login. Relay records usage and quota and never switches the account.",
      chip: "DOCUMENTED PATH",
      chipC: T.mu,
    }) +
    `<div style="display:flex;gap:10px">${field("Label (shown everywhere)", "Personal")}${field("Identifier shown as", "d•••@gmail.com")}</div>`,
  K.button("Cancel") + K.button("Continue", { kind: "primary" }),
);
const sheetB = sheet(
  "Connect — Codex setup",
  "Step 2 of 2 · OpenAI · Observe · “Dev”",
  "Point a T3 Codex instance at Relay",
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
      "Run any prompt in that T3 thread. Quota appears after the first response.",
    ) +
    `<div style="padding:10px 12px;border-radius:8px;background:color-mix(in srgb, ${T.am} 10%, white);${F}font-size:11px;line-height:16px;color:#5E3F0C">Not yet tested live. With a custom provider, T3's own Codex account and plan display may not appear for this instance.</div>`,
  K.button("Back") +
    K.button("Copy arguments") +
    K.button("Done", { kind: "primary" }),
);
const html4 =
  title(
    "Relay · final · main window",
    "Accounts & connect",
    "Provider → plan or credential → account. Each account carries your label, a masked identifier, its plan, and a labelled state with one fixing action where one exists. Connect asks for provider, mode and label first, so the same name appears in the tray, ledger and usage.",
  ) +
  mainWin("Main window — Accounts", "Accounts", accountsBody) +
  `<div layer-name="Connect flow" style="display:flex;gap:28px;align-items:flex-start">${sheetA}${sheetB}<div style="display:flex;flex-direction:column;gap:14px;width:250px">${cap("Provider picker", "Official marks identify the provider; choice cards and selected state use tonal fills, never outlines.")}${cap("Codex path", "Codex needs T3's Existing setup. A Claude router key would replace the subscription login, so subscription accounts are observed or opt-in pooled — never relabelled as “fully supported”.")}</div></div>`;

// ---------- 5 Usage, Through Relay & routing ----------
const bigSpark = (() => {
  const w = 560,
    h = 120,
    d = WEEK,
    max = 1.8,
    x = (i) => 20 + (i * (w - 40)) / 6,
    y = (v) => h - 18 - (v / max) * (h - 30);
  const pl = (a, s) =>
    `<polyline points="${a.map((v, i) => `${x(i).toFixed(1)},${y(v).toFixed(1)}`).join(" ")}" fill="none" ${s}/>`;
  return `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" style="flex-shrink:0"><line x1="20" y1="${h - 18}" x2="${w - 20}" y2="${h - 18}" stroke="${T.track}" stroke-width="1"/>${pl(d.prior, `stroke="${T.mu}" stroke-opacity="0.55" stroke-width="1.5" stroke-dasharray="3 3" stroke-linecap="round"`)}${pl(d.cur, `stroke="${T.ev}" stroke-width="2.25" stroke-linejoin="round" stroke-linecap="round"`)}<circle cx="${x(6)}" cy="${y(d.cur[6])}" r="3.5" fill="#FFFFFF" stroke="${T.ev}" stroke-width="2"/>${["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"].map((t, i) => `<text x="${x(i)}" y="${h - 3}" text-anchor="middle" font-size="10" fill="${T.mu}" font-family="SF Pro">${t}</text>`).join("")}</svg>`;
})();
const throughCard = surf(
  `<div style="display:flex;align-items:center;gap:10px;${F}"><div style="font-size:14px;font-weight:600;color:${G}">Through Relay</div><div style="font-size:12px;color:${L}">input + output tokens</div><div style="flex:1"></div>${K.seg("Week")}</div><div style="display:flex;align-items:flex-end;gap:32px"><div style="display:flex;flex-direction:column;gap:6px;width:300px;${F}"><div style="display:flex;align-items:baseline;gap:8px"><div style="font-size:34px;font-weight:700;letter-spacing:-0.02em;color:${G}">7.4M</div>${K.tag("↑ 17%", T.ev)}</div><div style="font-size:12px;line-height:18px;color:${L}">vs 6.3M over the same span last week<br>Mon 00:00 → Sun 09:41, local time</div><div style="display:flex;flex-direction:column;gap:4px;padding-top:6px;font-size:11px;color:${L}"><div style="display:flex;align-items:center;gap:6px"><div style="width:14px;height:2px;border-radius:1px;background:${T.ev}"></div>This week (to now)</div><div style="display:flex;align-items:center;gap:6px"><svg width="14" height="2"><line x1="0" y1="1" x2="14" y2="1" stroke="${T.mu}" stroke-opacity="0.7" stroke-width="1.5" stroke-dasharray="2.5 2"/></svg>Last week, same span</div></div></div>${bigSpark}<div style="flex:1;${F}font-size:11px;line-height:16px;color:${L}">Month compares Oct 1–18 with Sep 1–18. Missing days break the line; zero days sit on the baseline. All states: board 9.</div></div>`,
  "16px 18px",
);
const NUM = ["Input", "Output", "Cached", "Requests", "Errors"];
const uCols = `<div style="display:flex;align-items:center;gap:10px;padding:4px 12px;${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="width:240px;flex-shrink:0">Account</div>${NUM.map((t) => `<div style="width:80px;flex-shrink:0;text-align:right">${t}</div>`).join("")}<div style="flex:1;padding-left:16px">Share of tokens</div></div>`;
const uRow = (mark, lb, plan, v, share, shade, bold = false) =>
  `<div style="display:flex;align-items:center;gap:10px;height:40px;padding:0 12px;border-radius:9px;${shade ? `background:${W.inset};` : ""}${F}font-size:12px;color:${G};${bold ? "font-weight:600;" : ""}"><div style="width:240px;flex-shrink:0;display:flex;align-items:center;gap:8px">${box(mark, 16)}<div style="display:flex;flex-direction:column"><div style="font-weight:600;white-space:nowrap">${lb}</div><div style="font-size:10px;color:${L};font-weight:400;white-space:nowrap">${plan}</div></div></div>${v.map((x) => `<div style="width:80px;flex-shrink:0;text-align:right">${x}</div>`).join("")}<div style="flex:1;padding-left:16px;display:flex;align-items:center;gap:8px">${share == null ? "" : `<div style="flex:1;height:4px;border-radius:3px;background:${T.track};display:flex"><div style="width:${share}%;height:4px;border-radius:3px;background:${T.ev}"></div></div><div style="width:34px;text-align:right;font-size:11px;color:${L}">${share}%</div>`}</div></div>`;
const usageTable = surf(
  `<div style="display:flex;align-items:center;gap:10px;padding:0 4px;${F}"><div style="font-size:14px;font-weight:600;color:${G}">By account</div><div style="font-size:12px;color:${L}">measured by Relay · summed, never averaged</div><div style="flex:1"></div><div style="display:flex;padding:2px;border-radius:7px;background:${W.inset}">${["24 hours", "7 days", "30 days"].map((t) => `<div style="padding:2px 10px;border-radius:5px;${t === "7 days" ? `background:#FFFFFF;box-shadow:${T.lift};color:${G};font-weight:600;` : `color:${L};font-weight:500;`}${F}font-size:11px">${t}</div>`).join("")}</div></div>${uCols}` +
    uRow(
      CLAUDE(14),
      "Personal",
      "Claude Max · Anthropic",
      ["3.1M", "412K", "18.4M", "1,204", "3"],
      38,
      true,
    ) +
    uRow(
      CLAUDE(14),
      "Work",
      "Claude Max · Anthropic",
      ["2.2M", "288K", "11.0M", "806", "1"],
      26,
      false,
    ) +
    uRow(
      CLAUDE(14),
      "Secondary",
      "Claude Max · Anthropic",
      ["1.4M", "190K", "6.2M", "512", "14"],
      16,
      true,
    ) +
    uRow(
      ANTH(11, T.anth),
      "Key A · Key B",
      "API keys · Anthropic",
      ["0.4M", "52K", "1.1M", "140", "0"],
      5,
      false,
    ) +
    uRow(
      CHATGPT(14, T.gpt),
      "Dev",
      "ChatGPT Plus · Codex · OpenAI",
      ["0.9M", "160K", "2.8M", "331", "0"],
      11,
      true,
    ) +
    uRow(
      CHATGPT(14, T.gpt),
      "Team",
      "ChatGPT Plus · Codex · OpenAI",
      ["0.3M", "41K", "0.9M", "97", "2"],
      4,
      false,
    ) +
    uRow(
      "",
      "Total",
      "all accounts",
      ["8.3M", "1.14M", "40.4M", "3,090", "20"],
      null,
      true,
      true,
    ) +
    `<div style="padding:4px 4px 0;${F}font-size:11px;line-height:16px;color:${L}">Counts only requests that passed through Relay, so totals will not match provider dashboards.</div>`,
  "14px 12px",
);
const usageBody = `<div style="display:flex;flex-direction:column;gap:12px;padding:16px 20px 20px">${throughCard}${usageTable}</div>`;
const toggle = (on) =>
  `<div style="width:32px;height:20px;flex-shrink:0;border-radius:10px;background:${on ? T.ev : W.ctrl};display:flex;align-items:center;justify-content:${on ? "flex-end" : "flex-start"};padding:0 2px"><div style="width:16px;height:16px;border-radius:8px;background:#FFFFFF;box-shadow:0 1px 2px rgba(0,0,0,0.2)"></div></div>`;
const val = (t, w = 90) =>
  `<div style="width:${w}px;height:28px;flex-shrink:0;display:flex;align-items:center;padding:0 10px;border-radius:7px;background:${W.inset};${F}font-size:12px;color:${G}">${t}</div>`;
const segN = (opts, a) =>
  `<div style="display:flex;padding:2px;border-radius:7px;background:${W.inset}">${opts.map((o) => `<div style="padding:3px 10px;border-radius:5px;${o === a ? `background:#FFFFFF;box-shadow:${T.lift};color:${G};font-weight:600;` : `color:${L};font-weight:500;`}${F}font-size:11px">${o}</div>`).join("")}</div>`;
const setRow = (lb, sub, control) =>
  `<div style="display:flex;align-items:center;gap:16px;padding:8px 0;${F}"><div style="display:flex;flex-direction:column;gap:2px;flex:1"><div style="font-size:13px;color:${G}">${lb}</div>${sub ? `<div style="font-size:11px;line-height:16px;color:${L}">${sub}</div>` : ""}</div>${control}</div>`;
const capRow = (mark, prov, observe, pool, shade) =>
  `<div style="display:flex;align-items:center;gap:10px;height:34px;padding:0 10px;border-radius:8px;${shade ? `background:${W.inset};` : ""}${F}font-size:12px;color:${G}"><div style="flex:1;display:flex;align-items:center;gap:8px">${box(mark, 16)}${prov}</div><div style="width:170px;flex-shrink:0">${observe}</div><div style="width:190px;flex-shrink:0">${pool}</div></div>`;
const routingBody = `<div style="display:flex;gap:12px;padding:16px 20px 20px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:12px;flex:1">${surf(
  label("Pooled groups · new sessions") +
    setRow(
      "Strategy",
      "Most headroom first; when two are close, the one that resets sooner.",
      segN(["Quota-aware", "Round robin", "Priority"], "Quota-aware"),
    ) +
    setRow(
      "Running sessions stay on their account",
      "Only new sessions are placed by headroom. A session moves only if its account is exhausted.",
      toggle(true),
    ) +
    setRow("Skip an account below", "", val("5% left", 90)),
)}
${surf(label("Quota data") + setRow("Treat values as stale after", "Stale values show their age and are left out of averages.", val("15 min", 90)) + setRow("Poll quota while idle", "Off: values update only when requests pass through.", toggle(false)))}
${surf(label("Endpoint") + setRow("Local address", "", val("127.0.0.1:8317", 140)) + setRow("Open at login", "", toggle(true)))}</div>
<div style="display:flex;flex-direction:column;gap:12px;width:600px;flex-shrink:0">${surf(
  label("Capabilities in this build") +
    `<div style="display:flex;gap:10px;padding:0 10px;${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="flex:1">Credential</div><div style="width:170px">Observe</div><div style="width:190px">Pool</div></div>` +
    capRow(
      CLAUDE(14),
      "Claude subscription",
      K.tag("DOCUMENTED", T.mu),
      K.tag("OPT-IN · TERMS UNRESOLVED", T.am),
      true,
    ) +
    capRow(
      ANTH(11, T.anth),
      "Anthropic API key",
      K.tag("YES", T.mu),
      K.tag("AVAILABLE", T.ev),
      false,
    ) +
    capRow(
      CHATGPT(14, T.gpt),
      "ChatGPT / Codex subscription",
      K.tag("T3 EXISTING · UNTESTED", T.mu),
      K.tag("OPT-IN · TERMS UNRESOLVED", T.am),
      true,
    ) +
    capRow(
      OPENAI(15, T.gpt),
      "OpenAI API key",
      K.tag("YES", T.mu),
      K.tag("AVAILABLE", T.ev),
      false,
    ) +
    capRow(
      "",
      "Other providers",
      K.tag("NOT EVALUATED", T.mu),
      K.tag("NOT EVALUATED", T.mu),
      true,
    ) +
    `<div style="padding:4px 10px 0;${F}font-size:11px;line-height:16px;color:${L}">Statuses describe what this build does, not legal advice. Observed groups always follow their T3 instance.</div>`,
)}</div></div>`;
const html5 =
  title(
    "Relay · final · main window",
    "Usage, Through Relay & routing",
    "Usage leads with the Through Relay comparison (same component as the tray footer, larger), then per-account totals with the same marks and labels as the tray. Routing applies to pooled groups only: quota-aware placement of new sessions, running sessions sticky, idle polling off by default.",
  ) +
  mainWin("Main window — Usage", "Usage", usageBody) +
  mainWin("Main window — Routing", "Routing", routingBody);

// ---------- 6 States & numbers ----------
const bar6 = (pct, c, o = {}) =>
  o.hatch
    ? `<div style="width:120px;height:4px;flex-shrink:0;border-radius:3px;background-image:repeating-linear-gradient(135deg,${T.hatch} 0px,${T.hatch} 2px,transparent 2px,transparent 5px);background-color:${T.track}"></div>`
    : `<div style="width:120px;height:4px;flex-shrink:0;border-radius:3px;background:${T.track};display:flex"><div style="width:${pct}%;height:4px;border-radius:3px;background:${c};opacity:${o.op ?? 1}"></div></div>`;
const slot = (x) =>
  `<div style="width:150px;flex-shrink:0;display:flex">${x}</div>`;
const stRow = (name, sample, value, rule, i) =>
  `<div style="display:flex;align-items:center;gap:24px;padding:12px 16px;border-radius:10px;${i % 2 === 0 ? `background:${W.inset};` : ""}${F}"><div style="width:130px;flex-shrink:0;font-size:14px;font-weight:600;color:${G}">${name}</div>${sample}<div style="width:150px;flex-shrink:0;font-size:13px;font-weight:500;color:${G}">${value}</div><div style="flex:1;font-size:13px;line-height:19px;color:${L}">${rule}</div></div>`;
const states = [
  [
    "In use",
    slot(K.INUSE(2)),
    "Work",
    "Account currently carrying sessions. Running sessions stay on it while it has headroom. Relay never predicts which account a future session will get.",
  ],
  [
    "Healthy",
    bar6(72, T.ev),
    "72%",
    "Fresh value (under 15 min), 40% or more left.",
  ],
  [
    "Low",
    bar6(22, T.am),
    "22%",
    "Fresh value under 40% left. Pooled groups start preferring other accounts.",
  ],
  [
    "Exhausted",
    slot(K.EXH),
    `<span style="color:${T.em}">0% · resets 38m</span>`,
    "Provider reported the limit reached. Shows reset time when reported, otherwise “reset unknown”. Pooled: no new sessions until reset.",
  ],
  [
    "Stale",
    slot(K.STALE),
    `<span style="color:${L}">58% · 18m ago</span>`,
    "Value older than 15 min: dimmed bar with its age, left out of averages, counted as “n stale”.",
  ],
  [
    "Unknown",
    bar6(0, L, { hatch: true }),
    `<span style="color:${L}">— not reported</span>`,
    "No value for this window. Never shown as 0%, never averaged.",
  ],
  [
    "Disconnected",
    slot(DISC),
    `<span style="color:${L}">Reconnect</span>`,
    "T3 instance missing, auth rejected or key revoked. No quota shown; one action to fix.",
  ],
  [
    "Paused",
    slot(PAUSED),
    `<span style="color:${L}">Resume</span>`,
    "Set by you. Quota still shown; never chosen by the pool.",
  ],
  [
    "Group average",
    bar6(48, T.ev),
    "48% · avg · 3 fresh",
    "Mean over fresh accounts in one comparable group and window; the count is always printed. 5-hour and weekly windows, and different providers or plans, are never combined. Tokens are summed.",
  ],
];
const miniPop = (name, statusTag, inner, foot) =>
  `<div layer-name="${name}" style="display:flex;flex-direction:column;gap:8px;width:360px;flex-shrink:0;padding:8px;background:${T.pop};border-radius:16px;box-shadow:${T.shadow}"><div style="display:flex;align-items:center;gap:8px;padding:6px 8px 2px;${F}"><div style="font-size:15px;font-weight:600;color:${G}">Relay</div>${statusTag}<div style="flex:1"></div><div style="font-size:11px;color:${L}">127.0.0.1:8317</div></div>${inner}<div style="display:flex;flex-direction:column;gap:10px;padding:10px 12px;border-radius:12px;background:#FFFFFF">${foot}<div style="display:flex;align-items:center;gap:6px">${K.iconBtn(K.pauseSvg)}${K.iconBtn(K.gearSvg)}<div style="flex:1"></div>${K.openRelay}</div></div></div>`;
const pill = (t, c) =>
  `<div style="display:flex;align-items:center;gap:5px;padding:2px 8px 2px 6px;border-radius:10px;background:color-mix(in srgb, ${c} 14%, transparent);${F}"><div style="width:6px;height:6px;border-radius:3px;background:${c}"></div><div style="font-size:11px;font-weight:500;color:${c}">${t}</div></div>`;
const noTraffic = `<div style="display:flex;flex-direction:column;gap:4px;${F}"><div style="display:flex;align-items:center;gap:8px"><div style="font-size:11px;font-weight:600;color:${G}">Through Relay</div><div style="font-size:11px;color:${L}">tokens</div><div style="flex:1"></div>${K.seg("Week")}</div><div style="font-size:19px;font-weight:700;color:${G}">—</div><div style="font-size:10px;color:${L}">No requests through Relay yet</div></div>`;
const emptyPop = miniPop(
  "Popover — empty (first run)",
  pill("Running", T.ev),
  `<div style="display:flex;flex-direction:column;align-items:flex-start;gap:8px;padding:16px 14px;border-radius:12px;background:#FFFFFF;${F}"><div style="font-size:14px;font-weight:600;color:${G}">No accounts yet</div><div style="font-size:12px;line-height:17px;color:${L}">Connect a subscription or API key, or observe a T3 instance. Relay shows quota as soon as a request passes through.</div>${K.button("Connect an account…", { kind: "secondary" })}</div>`,
  noTraffic,
);
const errPop = miniPop(
  "Popover — not routing (port in use)",
  pill("Not routing", T.em),
  `<div style="display:flex;flex-direction:column;align-items:flex-start;gap:8px;padding:14px;border-radius:12px;background:color-mix(in srgb, ${T.em} 8%, #FFFFFF);${F}"><div style="font-size:13px;font-weight:600;color:${T.em}">Port 8317 is in use</div><div style="font-size:12px;line-height:17px;color:${L}">Another process holds 127.0.0.1:8317. Quota below is from before the stop and is marked stale.</div><div style="display:flex;gap:6px">${K.button("Try again")}${K.button("Change port…")}</div></div>${K.section(box(ANTH(10, T.anth), 14), "Anthropic", `<div style="display:flex;align-items:center;gap:8px;${F}">${box(CLAUDE(14), 16)}<div style="font-size:13px;font-weight:600;color:${G}">Claude Max</div><div style="font-size:11px;color:${L}">3 subscriptions</div><div style="flex:1"></div>${K.STALE}</div>` + K.win({ label: "5-hour", pct: 48, stale: true, meta: "22m ago" }))}`,
  K.through("Week"),
);
const html6 =
  title(
    "Relay · final · semantics",
    "States & numbers",
    "Every surface uses these rules. Bars show quota <b>left</b>. Status is a glyph plus a word; colour only reinforces it. Popover edge states keep the pinned footer and Open Relay.",
  ) +
  `<div style="display:flex;gap:40px;align-items:flex-start"><div layer-name="State rules" style="display:flex;flex-direction:column;gap:2px;width:820px;flex-shrink:0">${states.map((s, i) => stRow(...s, i)).join("")}</div><div style="display:flex;flex-direction:column;gap:24px;width:380px">${emptyPop}${cap("Empty · first run", "No groups to show; one action. The footer shows “—”, never 0.")}${errPop}${cap("Not routing", "Status pill turns red, cached values are labelled stale, and the fix is in place. Exhausted and disconnected accounts appear in the ledger (board 3) and Accounts (board 4).")}</div></div>`;

// ---------- 7 Native fallback menu ----------
const mi = (
  t,
  {
    k = "",
    dis = false,
    bold = false,
    ind = false,
    sub = false,
    hi = false,
  } = {},
  mode = "mac",
) =>
  `<div style="display:flex;align-items:center;gap:10px;height:${mode === "mac" ? 22 : 28}px;padding:0 10px 0 ${ind ? 24 : 10}px;border-radius:${mode === "mac" ? 5 : 6}px;${hi ? `background:${mode === "mac" ? "#2F6FDB" : "#3584E4"};` : ""}${F}font-size:13px;font-weight:${bold ? 600 : 400};color:${hi ? "#FFFFFF" : dis ? "#8A908D" : G}"><div style="flex:1;white-space:nowrap">${t}</div>${sub ? `<span style="color:${hi ? "#FFFFFF" : L}">›</span>` : ""}<div style="color:${hi ? "#FFFFFF" : L};font-size:12px">${k}</div></div>`;
const gap = `<div style="height:6px"></div>`;
const menuItems = (m) =>
  [
    mi("Relay — Running", { dis: true }, m),
    mi("Claude Max · 5-hour 48% · weekly 71%", { dis: true }, m),
    mi("Work — in use by 2 sessions", { ind: true, dis: true }, m),
    mi("ChatGPT Plus · Codex · 5-hour 22% · 1 stale", { dis: true }, m),
    mi("Dev — in use by 1 session", { ind: true, dis: true }, m),
    gap,
    mi(
      "Open Relay",
      { k: m === "mac" ? "⌘O" : "Ctrl+O", bold: true, hi: true },
      m,
    ),
    mi("Pause routing", {}, m),
    mi("Settings…", { k: m === "mac" ? "⌘," : "Ctrl+," }, m),
    gap,
    mi("Quit Relay", { k: m === "mac" ? "⌘Q" : "Ctrl+Q" }, m),
  ].join("");
const macMenu = `<div layer-name="macOS native menu" style="display:flex;flex-direction:column;width:330px;padding:5px;border-radius:10px;background:rgba(246,246,246,0.96);box-shadow:0 12px 32px rgba(0,0,0,0.22)">${menuItems("mac")}</div>`;
const linMenu = `<div layer-name="Linux dbusmenu (GNOME AppIndicator)" style="display:flex;flex-direction:column;width:360px;padding:6px;border-radius:12px;background:#FFFFFF;box-shadow:0 12px 32px rgba(0,0,0,0.25)">${menuItems("lin")}</div>`;
const html7 =
  title(
    "Relay · final · fallback",
    "Native menu fallback",
    "Used when the rich popover is unavailable: right-click on Linux, hosts that do not send Activate, or when the user prefers a plain menu for assistive technology. The operating system draws it, so Relay supplies text, order, shortcuts and enabled state only. Open Relay is the default item; no account is predicted.",
  ) +
  `<div style="display:flex;gap:48px;align-items:flex-start"><div style="display:flex;flex-direction:column;gap:14px"><div style="display:flex;flex-direction:column;align-items:flex-end;width:420px;padding:0 0 28px;border-radius:12px;background:${T.desk};overflow:hidden">${K.menubar("Sun 18 Oct  9:41")}<div style="padding:6px 16px 0">${macMenu}</div></div>${cap("macOS · NSMenu via tray icon", "System font and material; separators become spacing where the OS allows. Status rows are disabled text so VoiceOver reads them in order.")}</div>
<div style="display:flex;flex-direction:column;gap:14px"><div style="display:flex;flex-direction:column;align-items:flex-end;width:440px;padding:0 0 28px;border-radius:12px;background:#3A4541;overflow:hidden"><div style="display:flex;align-items:center;justify-content:flex-end;gap:12px;width:100%;box-sizing:border-box;height:30px;padding:0 12px;background:#141716;${F}font-size:12px;color:#EDEFEE"><div style="display:flex;align-items:center;justify-content:center;width:24px;height:24px;border-radius:5px;background:rgba(255,255,255,0.14)">${K.trayIcon("#EDEFEE")}</div><div>Sun 09:41</div></div><div style="padding:8px 12px 0">${linMenu}</div></div>${cap("Linux · StatusNotifierItem + dbusmenu (ksni)", "Rendered by the panel (GNOME needs the AppIndicator extension; KDE native). Shortcut labels are shown as hints; the panel may not display them. Highlight colour belongs to the desktop theme.")}</div>
<div style="display:flex;flex-direction:column;gap:14px;width:300px">${cap("Item order", "1 status · 2 group summaries (comparable windows only) · 3 accounts in use · 4 Open Relay · 5 Pause routing · 6 Settings · 7 Quit.")}${cap("Refresh", "Rebuilt from the same snapshot as the popover at most once per second; never triggers a quota request.")}${cap("Not shown", "Logos (menus may not support images on every host), sparkline, predicted accounts.")}</div></div>`;

// ---------- 8 Buttons, interaction & motion ----------
const LD = [
  ["Light", Lk, "#FFFFFF"],
  ["Dark", Dk, DK.surf],
];
const ST = [
  ["Rest", "rest"],
  ["Hover", "hover"],
  ["Pressed", "pressed"],
  ["Keyboard focus", "focus"],
  ["Disabled", "disabled"],
];
const strip = LD.map(
  ([nm, k, bg]) =>
    `<div style="display:flex;flex-direction:column;gap:12px;padding:16px 18px;border-radius:14px;background:${bg}"><div style="${F}font-size:11px;font-weight:600;color:${k.T.mu}">${nm}</div>${[
      [
        "Primary",
        (s) => k.button("Open Relay ↗", { kind: "primary", state: s }),
      ],
      ["Secondary", (s) => k.button("Refresh quota", { state: s })],
      ["Icon", (s) => k.button("", { icon: k.gearSvg, state: s })],
    ]
      .map(
        ([rl, f]) =>
          `<div style="display:flex;align-items:center;gap:0;${F}"><div style="width:84px;flex-shrink:0;font-size:11px;color:${k.T.mu}">${rl}</div>${ST.map(([, s]) => `<div style="width:132px;flex-shrink:0;display:flex">${f(s)}</div>`).join("")}</div>`,
      )
      .join("")}</div>`,
).join("");
const stripHead = `<div style="display:flex;padding:0 18px;${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}"><div style="width:84px"></div>${ST.map(([t]) => `<div style="width:132px">${t}</div>`).join("")}</div>`;
const rowHead = (lb, sty) =>
  `<div style="display:flex;flex-direction:column;gap:6px;width:200px;${F}"><div style="font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}">${lb}</div><div style="display:flex;align-items:center;gap:8px;height:28px;padding:0 8px;border-radius:7px;${sty}">${box(CLAUDE(14), 16)}<div style="font-size:13px;font-weight:600;color:${G}">Claude Max</div><div style="font-size:11px;color:${L}">3</div></div></div>`;
const rowStates = `<div style="display:flex;gap:16px;flex-wrap:wrap">${rowHead("Rest", "")}${rowHead("Hover", `background:${W.inset};`)}${rowHead("Pressed", `background:${W.ctrl};`)}${rowHead("Keyboard focus", `background:color-mix(in srgb, ${T.ev} 14%, transparent);`)}${rowHead("Disabled", "opacity:0.45;")}</div>`;
const tbl = (rs, cols) =>
  `<div style="display:flex;flex-direction:column;gap:2px">${rs.map((r, i) => `<div style="display:flex;gap:16px;padding:8px 12px;border-radius:8px;${i === 0 ? "" : i % 2 === 1 ? `background:${W.inset};` : ""}${F}font-size:12px;line-height:18px;color:${i === 0 ? L : G};${i === 0 ? "font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;" : ""}">${r.map((c, j) => `<div style="${cols[j] ? `width:${cols[j]}px;flex-shrink:0` : "flex:1"};${j > 0 && i > 0 ? `color:${L}` : ""}">${c}</div>`).join("")}</div>`).join("")}</div>`;
const motion = tbl(
  [
    ["Interaction", "Motion", "Reduced motion"],
    [
      "Pointer-down",
      "Pressed surface on the same frame: sinks 1 px with inset shade, no layout shift. Release returns in 120 ms, ease-out (0.23, 1, 0.32, 1). Drag-off cancels.",
      "Shade only, no movement",
    ],
    [
      "Keyboard Space / Return",
      "Static pressed shade for one beat, then the action. No spatial movement.",
      "Same",
    ],
    ["Hover", "Surface tint in 120 ms ease-out; pointer only.", "Instant"],
    [
      "Popover open",
      "160 ms ease-out: opacity 0→1, 4 px drop and 98%→100% scale from the tray anchor. Fade only when the compositor places the window (GNOME).",
      "120 ms fade",
    ],
    [
      "Popover close",
      "120 ms, the reverse of open, starting from the current presentation (interruptible).",
      "Fade",
    ],
    [
      "Group expand / collapse",
      "Height and chevron together, 180 ms ease-out; reversible mid-way from the current height.",
      "Instant",
    ],
    [
      "Week / Month",
      "Selected pill slides 160 ms; sparkline crossfades 120 ms. Lines never draw on.",
      "Instant swap",
    ],
    [
      "Telemetry updates",
      "Values, bars and the sparkline marker update in place: no counting, interpolation, staggers or chasing.",
      "Same",
    ],
  ],
  [170, null, 150],
);
const accept = tbl(
  [
    ["Acceptance (verify with native recordings)", "Pass when"],
    [
      "Press latency",
      "Pressed visual within 1 frame of pointer-down (≤ 16.7 ms at 60 Hz, ≤ 8.3 ms at 120 Hz).",
    ],
    [
      "Frame pacing",
      "Open, close and expand show at most 1 dropped frame each; p95 frame time within the refresh budget on macOS (ProMotion and 60 Hz) and Linux (Wayland and X11).",
    ],
    [
      "Cancellation",
      "Esc, outside click or re-click mid-animation reverses from the current values with no jump; Esc closes within 120 ms.",
    ],
    [
      "Bounds",
      "Every animation ≤ 300 ms. No scale(0), ease-in, overshoot or bounce. Surrounding layout moves 0 px on press.",
    ],
    [
      "Reduced motion",
      "macOS Reduce Motion and GNOME/GTK enable-animations=false switch to the right-hand column.",
    ],
    [
      "Status",
      "Not yet verified. Static boards cannot show smoothness; record 120 fps (macOS) and 60 fps (Linux) after implementation.",
    ],
  ],
  [260, null],
);
const keys = tbl(
  [
    ["Key", "Action"],
    [
      "Click tray icon / global shortcut",
      "Open; focus moves to the first group header",
    ],
    [
      "Tab / Shift-Tab",
      "Group headers → account cards → Pause → Settings → Open Relay; wraps inside the popover",
    ],
    ["⌘O (Ctrl+O)", "Open Relay on Quotas, then close the popover"],
    ["↑ ↓ · → ←", "Move between rows · expand / collapse"],
    [
      "Esc, click outside, click icon",
      "Close; focus returns to the previous app",
    ],
    ["⌘, (Ctrl+,)", "Settings"],
  ],
  [220, null],
);
const sr = tbl(
  [
    ["Element", "Accessible name (VoiceOver; AT-SPI on Linux)"],
    [
      "Group header",
      "“Claude Max, 3 subscriptions, pooled. 5-hour 48 percent left, average of 3 fresh accounts. Collapsed, button.”",
    ],
    [
      "Account card",
      "“Work, d…@acme.co, in use by 2 sessions. 5-hour 64 percent left, resets in 3 hours.”",
    ],
    [
      "Unknown / stale",
      "“Weekly, not reported.” · “Team, 58 percent, stale, 18 minutes ago, not in average.”",
    ],
    [
      "Footer",
      "“Open Relay, opens the main window on Quotas”; “Pause routing”; “Settings”. Icon buttons carry names and tooltips.",
    ],
  ],
  [140, null],
);
const sec8 = (h, inner, w) =>
  `<div style="display:flex;flex-direction:column;gap:12px;width:${w}px;flex-shrink:0"><div style="${F}font-size:15px;font-weight:600;color:${G}">${h}</div>${inner}</div>`;
const html8 =
  title(
    "Relay · final · behaviour spec",
    "Buttons, interaction & motion",
    "Tactile but quiet: buttons rest slightly raised and press in about 1 px, with no layout shift; focus and selection are filled tints, never outlines. Motion is short, interruptible and ease-out, adapted for native GPUI (Apple HIG + fluid-interface guidance, emil-design-eng). Targets: 28 pt controls; the whole group row is clickable.",
  ) +
  `<div style="display:flex;flex-direction:column;gap:10px">${sec8("Button states", "", 1240)}${stripHead}<div style="display:flex;flex-direction:column;gap:12px;padding:16px;border-radius:16px;background:${T.canvas};width:fit-content">${strip}</div></div>` +
  sec8("Row states (group header)", rowStates, 1240) +
  `<div style="display:flex;gap:40px;align-items:flex-start">${sec8("Motion", motion, 700)}${sec8("Acceptance criteria", accept, 500)}</div>` +
  `<div style="display:flex;gap:40px;align-items:flex-start">${sec8("Keyboard & dismissal", keys, 600)}${sec8("Screen reader", sr + `<div style="${F}font-size:11px;line-height:16px;color:${L}">GPUI accessibility-tree support must be verified before implementation; a gap is a blocker to resolve, not a reason to drop these semantics.</div>`, 600)}</div>`;

// ---------- run ----------
const out = {};
const mk = async (key, name, html, w) => {
  out[key] = idOf(
    await tools.paper.create_artboard({
      fileId,
      pageId: PAGE,
      name,
      styles: board(w),
    }),
  );
  await tools.paper.write_html({
    fileId,
    targetNodeId: out[key],
    mode: "insert-children",
    html,
  });
};
await mk("a4", "4 · Accounts & connect", html4, 1340);
await mk("a5", "5 · Usage, Through Relay & routing", html5, 1340);
await mk("a6", "6 · States & numbers", html6, 1340);
await mk("a7", "7 · Native menu fallback", html7, 1340);
await mk("a8", "8 · Buttons, interaction & motion", html8, 1340);
return out;
