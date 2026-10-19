// Executor `execute` program: rebuilds board 09 (main window · Quotas) — borderless tonal style, official SVG marks.
// Paper design tooling only — not application code. All values and identities are design samples.
const fileId = "01M58Z151ZVWZ4VVK921R3Q5J9";
const PAGE = "p-2-0"; // Final Designs page; archived drafts stay on the Drafts / Archive page
const F = `font-family:var(--font-ui);font-variant-numeric:tabular-nums;`;
const G = "#1C1F1E",
  L = "#646A67",
  EV = "#1F6B4F",
  AM = "#9A6614",
  EM = "#B4432F";
const CANVAS = "#F1F2F1",
  CHROME = "#E9EBEA",
  SURF = "#FFFFFF",
  CTRL = "#E4E7E5",
  INSET = "#F5F6F5",
  TRACK = "#E8EBE9";
const idOf = (res) =>
  JSON.stringify(res).match(/\\"id\\": \\"([0-9A-Z]+-[0-9]+)\\"/)[1];
// Official marks (design/assets/logos; see logo-sources.md)
const ANTH = (h = 12, c = "#141413") =>
  `<svg width="${Math.round((h * 92) / 64)}" height="${h}" viewBox="0 0 92 64" style="flex-shrink:0"><path d="M65.92 0H51.9314L77.44 64H91.4286L65.92 0Z" fill="${c}"/><path d="M25.5086 0L0 64H14.2629L19.4798 50.56H46.166L51.3829 64H65.6457L40.1371 0H25.5086ZM24.0933 38.6743L32.8229 16.1829L41.5525 38.6743H24.0933Z" fill="${c}"/></svg>`;
const CLAUDE = (s = 16) =>
  `<svg width="${s}" height="${s}" viewBox="0 0 94 94" style="flex-shrink:0"><path d="M18.7657 62.4437L37.1822 52.1167L37.4857 51.2122L37.1822 50.7085H36.2715L33.1852 50.5208L22.6615 50.2391L13.5545 49.8636L4.70044 49.3942L2.47428 48.9248L0.399902 46.1553L0.602281 44.794L2.47428 43.5266L5.15579 43.7613L11.0754 44.1837L19.98 44.794L26.4055 45.1695L35.9679 46.1553H37.4857L37.6881 45.545L37.1822 45.1695L36.7774 44.794L27.5692 38.5508L17.6021 31.9791L12.3908 28.1769L9.60812 26.2524L8.19147 24.4686L7.58433 20.5256L10.1141 17.7091L13.5545 17.9438L14.4146 18.1785L17.9056 20.8542L25.343 26.6279L35.0572 33.7629L36.4739 34.9364L37.0443 34.5514L37.1316 34.2792L36.4739 33.1996L31.212 23.6706L25.596 13.9539L23.0663 9.91695L22.4086 7.52296C22.1538 6.51831 22.0038 5.68714 22.0038 4.65957L24.8877 0.716544L26.5067 0.200195L30.4025 0.716544L32.0215 2.12477L34.4501 7.66379L38.3458 16.3478L44.4172 28.1769L46.188 31.6975L47.1493 34.9364L47.5035 35.9222H48.1106V35.3589L48.6166 28.6933L49.5273 20.5256L50.438 10.0108L50.7415 7.05356L52.2088 3.48605L55.1433 1.56148L57.42 2.64112L59.292 5.31674L59.039 7.05356L57.926 14.2824L55.7504 25.5952L54.3337 33.1996H55.1433L56.1046 32.2138L59.9497 27.1442L66.3752 19.0704L69.2085 15.8784L72.5478 12.3579L74.6728 10.668H78.7203L81.6548 15.0804L80.3394 19.6337L76.1906 24.8911L72.7502 29.3504L67.8172 35.9595L64.7562 41.2734L65.0307 41.7118L65.7681 41.6489L76.8989 39.255L82.9197 38.1753L90.1041 36.9549L93.3422 38.457L93.6963 40.006L92.4315 43.151L84.7411 45.0287L75.7353 46.8594L62.3244 50.0164L62.1759 50.1358L62.3512 50.3958L68.399 50.9432L70.9794 51.084H77.3037L89.0922 51.9759L92.1785 53.9944L93.9999 56.4822L93.6963 58.4068L88.9404 60.8008L82.5655 59.2987L67.6401 55.7312L62.5301 54.4638H61.8217V54.8862L66.0717 59.064L73.9139 66.1051L83.6786 75.2116L84.1845 77.4648L82.9197 79.2485L81.6042 79.0608L73.0032 72.5829L69.6639 69.6726L62.1759 63.3356H61.67V63.9928L63.3902 66.5276L72.5478 80.2812L73.0032 84.5059L72.3454 85.8672L69.9675 86.7121L67.3871 86.2427L61.9735 78.6852L56.4587 70.2359L52.0064 62.6315L51.4687 62.971L48.8189 91.2654L47.6047 92.7206L44.7714 93.8002L42.3934 92.0164L41.1286 89.1061L42.3934 83.3324L43.9113 75.8219L45.1255 69.8604L46.2386 62.4437L46.9184 59.9661L46.8583 59.8003L46.3153 59.8916L40.7238 67.5603L32.2239 79.0608L25.4948 86.2427L23.8758 86.8999L21.0931 85.4447L21.3461 82.863L22.9145 80.5629L32.2239 68.7338L37.8399 61.3641L41.4594 57.1337L41.4242 56.5218L41.2244 56.5048L16.489 72.6299L12.0873 73.1932L10.1647 71.4094L10.4176 68.4991L11.3283 67.5603L18.7657 62.4437Z" fill="#D97757"/></svg>`;
const CHATGPT = (s = 16, c = "#000000") =>
  `<svg width="${s}" height="${s}" viewBox="0 0 21 21" style="flex-shrink:0"><path d="M11.8102 19.1625C11.2327 19.1625 10.6841 19.0528 10.1643 18.8334C9.64459 18.6139 9.18259 18.3078 8.77834 17.9151C8.33944 18.0653 7.88322 18.1404 7.40967 18.1404C6.63582 18.1404 5.91972 17.9498 5.26137 17.5686C4.60302 17.1875 4.07172 16.6677 3.66747 16.0094C3.27477 15.351 3.07842 14.6176 3.07842 13.8091C3.07842 13.4742 3.12462 13.1103 3.21702 12.7176C2.75502 12.2903 2.39697 11.7994 2.14287 11.245C1.88877 10.679 1.76172 10.09 1.76172 9.47785C1.76172 8.85415 1.89454 8.25355 2.16019 7.67605C2.42584 7.09855 2.79544 6.6019 3.26899 6.1861C3.75409 5.75875 4.31427 5.46422 4.94952 5.30252C5.07657 4.64417 5.34222 4.05512 5.74647 3.53537C6.16227 3.00407 6.67047 2.58827 7.27107 2.28797C7.87167 1.98767 8.51269 1.83752 9.19414 1.83752C9.77164 1.83752 10.3203 1.94725 10.84 2.1667C11.3598 2.38615 11.8218 2.69222 12.226 3.08492C12.6649 2.93477 13.1211 2.8597 13.5947 2.8597C14.3685 2.8597 15.0846 3.05027 15.743 3.43142C16.4013 3.81257 16.9269 4.33232 17.3196 4.99067C17.7238 5.64902 17.9259 6.38245 17.9259 7.19095C17.9259 7.5259 17.8797 7.88973 17.7873 8.28242C18.2493 8.70977 18.6074 9.20643 18.8615 9.77238C19.1156 10.3268 19.2426 10.91 19.2426 11.5222C19.2426 12.1459 19.1098 12.7465 18.8442 13.324C18.5785 13.9015 18.2031 14.4039 17.718 14.8313C17.2445 15.2471 16.6901 15.5358 16.0548 15.6975C15.9278 16.3559 15.6564 16.9449 15.2406 17.4647C14.8363 17.996 14.3339 18.4118 13.7333 18.7121C13.1327 19.0124 12.4917 19.1625 11.8102 19.1625ZM7.53094 16.9969C8.10844 16.9969 8.61087 16.8756 9.03822 16.6331L12.2953 14.762C12.4108 14.6811 12.4686 14.5714 12.4686 14.4328V12.9429L8.27592 15.351C8.02182 15.5012 7.76772 15.5012 7.51362 15.351L4.23919 13.4626C4.23919 13.4973 4.23342 13.5377 4.22187 13.5839C4.22187 13.6301 4.22187 13.6994 4.22187 13.7918C4.22187 14.3808 4.36047 14.9237 4.63767 15.4203C4.92642 15.9054 5.32489 16.2866 5.83309 16.5638C6.34129 16.8525 6.90724 16.9969 7.53094 16.9969ZM7.70419 14.1729C7.77349 14.2076 7.83702 14.2249 7.89477 14.2249C7.95252 14.2249 8.01027 14.2076 8.06802 14.1729L9.36739 13.4279L5.19207 11.0024C4.93797 10.8523 4.81092 10.6271 4.81092 10.3268V6.56725C4.23342 6.82135 3.77142 7.21405 3.42492 7.74535C3.07842 8.2651 2.90517 8.8426 2.90517 9.47785C2.90517 10.0438 3.04954 10.5866 3.33829 11.1064C3.62704 11.6261 4.00242 12.0189 4.46442 12.2845L7.70419 14.1729ZM11.8102 18.0191C12.4224 18.0191 12.9768 17.8805 13.4734 17.6033C13.9701 17.3261 14.3628 16.9449 14.6515 16.4598C14.9403 15.9747 15.0846 15.4319 15.0846 14.8313V11.0891C15.0846 10.9505 15.0269 10.8465 14.9114 10.7772L13.5947 10.0149V14.8486C13.5947 15.1489 13.4676 15.3741 13.2135 15.5243L9.93912 17.4127C10.5051 17.817 11.1288 18.0191 11.8102 18.0191ZM12.4686 11.6781V9.32192L10.5108 8.21312L8.53579 9.32192V11.6781L10.5108 12.7869L12.4686 11.6781ZM7.40967 6.15145C7.40967 5.85115 7.53672 5.62592 7.79082 5.47577L11.0652 3.58735C10.4993 3.1831 9.87559 2.98097 9.19414 2.98097C8.58199 2.98097 8.02759 3.11957 7.53094 3.39677C7.03429 3.67397 6.64159 4.05512 6.35284 4.54022C6.07564 5.02532 5.93704 5.56817 5.93704 6.16878V9.89365C5.93704 10.0323 5.99479 10.142 6.11029 10.2228L7.40967 10.9851V6.15145ZM16.2108 14.4328C16.7883 14.1787 17.2445 13.786 17.5794 13.2547C17.9259 12.7234 18.0992 12.1459 18.0992 11.5222C18.0992 10.9562 17.9548 10.4134 17.6661 9.89365C17.3773 9.3739 17.0019 8.9812 16.5399 8.71555L13.3002 6.84445C13.2309 6.79825 13.1673 6.78092 13.1096 6.79248C13.0518 6.79248 12.9941 6.8098 12.9363 6.84445L11.637 7.5721L15.8296 10.0149C15.9567 10.0842 16.0491 10.1766 16.1068 10.2921C16.1761 10.3961 16.2108 10.5231 16.2108 10.6733V14.4328ZM12.7284 5.6317C12.9825 5.47 13.2366 5.47 13.4907 5.6317L16.7825 7.55477C16.7825 7.47393 16.7825 7.36997 16.7825 7.24292C16.7825 6.68852 16.6439 6.163 16.3667 5.66635C16.101 5.15815 15.7141 4.7539 15.2059 4.4536C14.7093 4.1533 14.1318 4.00315 13.4734 4.00315C12.8959 4.00315 12.3935 4.12442 11.9661 4.36697L8.70904 6.23808C8.59354 6.31892 8.53579 6.42865 8.53579 6.56725V8.0572L12.7284 5.6317Z" fill="${c}"/></svg>`;

const OPENAI = (s = 14, c = "#000000") =>
  `<svg width="${s}" height="${s}" viewBox="0 0 32 32" style="flex-shrink:0"><path d="M29.71,13.09A8.09,8.09,0,0,0,20.34,2.68a8.08,8.08,0,0,0-13.7,2.9A8.08,8.08,0,0,0,2.3,18.9,8,8,0,0,0,3,25.45a8.08,8.08,0,0,0,8.69,3.87,8,8,0,0,0,6,2.68,8.09,8.09,0,0,0,7.7-5.61,8,8,0,0,0,5.33-3.86A8.09,8.09,0,0,0,29.71,13.09Zm-12,16.82a6,6,0,0,1-3.84-1.39l.19-.11,6.37-3.68a1,1,0,0,0,.53-.91v-9l2.69,1.56a.08.08,0,0,1,.05.07v7.44A6,6,0,0,1,17.68,29.91ZM4.8,24.41a6,6,0,0,1-.71-4l.19.11,6.37,3.68a1,1,0,0,0,1,0l7.79-4.49V22.8a.09.09,0,0,1,0,.08L13,26.6A6,6,0,0,1,4.8,24.41ZM3.12,10.53A6,6,0,0,1,6.28,7.9v7.57a1,1,0,0,0,.51.9l7.75,4.47L11.85,22.4a.14.14,0,0,1-.09,0L5.32,18.68a6,6,0,0,1-2.2-8.18Zm22.13,5.14-7.78-4.52L20.16,9.6a.08.08,0,0,1,.09,0l6.44,3.72a6,6,0,0,1-.9,10.81V16.56A1.06,1.06,0,0,0,25.25,15.67Zm2.68-4-.19-.12-6.36-3.7a1,1,0,0,0-1.05,0l-7.78,4.49V9.2a.09.09,0,0,1,0-.09L19,5.4a6,6,0,0,1,8.91,6.21ZM11.08,17.15,8.38,15.6a.14.14,0,0,1-.05-.08V8.1a6,6,0,0,1,9.84-4.61L18,3.6,11.61,7.28a1,1,0,0,0-.53.91ZM12.54,14,16,12l3.47,2v4L16,20l-3.47-2Z" fill="${c}"/></svg>`;
const box = (svg, s = 18) =>
  `<div style="width:${s}px;height:${s}px;flex-shrink:0;display:flex;align-items:center;justify-content:center">${svg}</div>`;
const lvl = (p) => (p < 15 ? EM : p < 40 ? AM : EV);
const tag = (t, c) =>
  `<div style="flex-shrink:0;width:fit-content;padding:2px 7px;border-radius:5px;background:color-mix(in srgb, ${c} 12%, transparent);${F}font-size:9px;font-weight:600;letter-spacing:0.06em;color:${c};white-space:nowrap">${t}</div>`;
const hatch = `background-image:repeating-linear-gradient(135deg,#D5D9D7 0px,#D5D9D7 2px,transparent 2px,transparent 5px);background-color:${TRACK}`;
const bar = (pct, { stale = false, h = 4 } = {}) =>
  pct == null
    ? `<div style="height:${h}px;border-radius:3px;${hatch}"></div>`
    : `<div style="height:${h}px;border-radius:3px;background:${TRACK};display:flex;overflow:hidden"><div style="width:${pct}%;height:${h}px;border-radius:3px;background:${lvl(pct)};opacity:${stale ? 0.35 : 1}"></div></div>`;
const segs = (vals) =>
  `<div style="display:flex;gap:5px">${vals.map((v) => `<div style="flex:1">${bar(v.p, { stale: v.stale, h: 5 })}</div>`).join("")}</div>`;
// Tactile button: rest = soft gradient + short shadow; disabled = flat and quiet (see board 08 state strip).
const btn = (t, { disabled = false } = {}) =>
  `<div style="display:flex;align-items:center;gap:5px;height:28px;padding:0 11px;border-radius:7px;${disabled ? "background:#F4F5F4;opacity:0.45;" : "background:linear-gradient(180deg, #FFFFFF, #F0F2F1);box-shadow:0 1px 2px rgba(28,31,30,0.14), inset 0 1px 0 rgba(255,255,255,0.7);"}${F}font-size:12px;font-weight:500;color:${G};white-space:nowrap">${t}</div>`;
const select = (k, v) =>
  `<div style="display:flex;align-items:center;gap:8px;height:30px;padding:0 11px;border-radius:7px;background:${CTRL};${F}font-size:12px;color:${L}">${k}<span style="color:${G};font-weight:500">${v}</span><span>▾</span></div>`;
const lights = `<div style="display:flex;gap:8px;width:60px">${["#FF5F57", "#FEBC2E", "#28C840"].map((c) => `<div style="width:12px;height:12px;border-radius:6px;background:${c}"></div>`).join("")}</div>`;
const tabs = `<div style="display:flex;padding:2px;border-radius:8px;background:${CTRL}">${["Quotas", "Accounts", "Usage", "Routing"].map((t, i) => `<div style="padding:4px 14px;border-radius:6px;${i === 0 ? `background:${SURF};` : ""}${F}font-size:12px;font-weight:${i === 0 ? 600 : 500};color:${i === 0 ? G : L}">${t}</div>`).join("")}</div>`;
const chip = (mark, label, n, active = false) =>
  `<div style="display:flex;align-items:center;gap:7px;height:30px;padding:0 11px;border-radius:7px;${active ? `background:${SURF};` : ""}${F}font-size:13px;font-weight:${active ? 600 : 500};color:${G}">${mark}${label}<span style="font-size:11px;color:${L};font-weight:500">${n}</span></div>`;
const filters = `<div style="display:flex;align-items:center;gap:2px;padding:14px 20px 0">${chip(box(`<span style="color:${L};font-size:12px">◫</span>`, 16), "All", 8, true)}${chip(box(ANTH(10), 16), "Anthropic", 6)}${chip(box(OPENAI(14), 16), "OpenAI", 2)}<div style="margin-left:12px;${F}font-size:11px;color:${L}">Configured providers only · logos do not imply other integrations</div></div>`;
const search = `<div style="display:flex;align-items:center;gap:8px;padding:10px 20px 4px"><div style="display:flex;align-items:center;gap:8px;width:360px;height:30px;padding:0 11px;border-radius:7px;background:${CTRL};${F}font-size:13px;color:#7D837F"><svg width="13" height="13" viewBox="0 0 13 13"><circle cx="5.5" cy="5.5" r="4" fill="none" stroke="#7D837F" stroke-width="1.4"/><path d="M8.6 8.6 L12 12" stroke="#7D837F" stroke-width="1.4" stroke-linecap="round"/></svg>Search by label, email or credential file</div><div style="flex:1"></div>${select("View", "Ledger")}${select("Sort", "Default")}</div>`;

// shared summary surface: contiguous provider columns
const capacity = ({ label, have, of, meta, vals, resets, big = true }) =>
  `<div style="display:flex;flex-direction:column;gap:8px;${F}"><div style="display:flex;align-items:baseline;gap:6px"><div style="font-size:12px;color:${L}">${label}</div><div style="flex:1"></div><div style="font-size:11px;color:${L}">${meta}</div></div><div style="display:flex;align-items:baseline;gap:6px"><div style="font-size:${big ? 28 : 22}px;font-weight:700;letter-spacing:-0.02em;color:${G}">${have}</div><div style="font-size:12px;color:${L}">of ${of} left</div></div>${segs(vals)}<div style="font-size:11px;color:${L}">${resets}</div></div>`;
const colHead = (mark, name, count) =>
  `<div style="display:flex;align-items:center;gap:8px;${F}">${mark}<div style="font-size:14px;font-weight:600;color:${G}">${name}</div><div style="flex:1"></div><div style="font-size:11px;color:${L}">${count}</div></div>`;
const short = (text) =>
  `<div style="${F}font-size:11px;color:${L}"><span style="font-weight:600;color:${G}">5-hour</span> · ${text}</div>`;
const model = (inner) =>
  `<div style="display:flex;flex-direction:column;gap:10px;padding:12px;border-radius:10px;background:${INSET}"><div style="display:flex;align-items:center;gap:6px;${F}font-size:11px;font-weight:600;color:${G}"><span style="color:${L}">▾</span>Model limit · Fable 5 · 7-day<div style="flex:1"></div><span style="color:${L};font-weight:500">Hide</span></div>${inner}</div>`;
const col = (inner) =>
  `<div style="display:flex;flex-direction:column;gap:14px;flex:1;min-width:0">${inner}</div>`;
const summary = `<div layer-name="Provider summary" style="display:flex;gap:44px;margin:14px 20px 0;padding:20px 22px;border-radius:14px;background:${SURF}">
${col(colHead(box(CLAUDE(16)), "Claude Max", "3 subscriptions") + capacity({ label: "Weekly limit", have: "212%", of: "300%", meta: "avg 71% · 3 fresh", vals: [{ p: 88 }, { p: 70 }, { p: 54 }], resets: `<span style="color:${G};font-weight:500">Resets in 4d</span> · Oct 22, 09:00 · others by Oct 24, 18:00` }) + short(`145% of 300% · avg 48% · first reset in 38m`) + model(capacity({ label: "7-day", have: "292%", of: "300%", meta: "avg 97% · 3 fresh", vals: [{ p: 100 }, { p: 92 }, { p: 100 }], resets: `<span style="color:${G};font-weight:500">Resets in 4d</span> · Oct 22, 09:00 · others by Oct 24`, big: false })))}
${col(colHead(box(ANTH(11)), "API keys", "2 keys") + capacity({ label: "Tokens per minute", have: "152%", of: "200%", meta: "avg 76% · 2 fresh", vals: [{ p: 84 }, { p: 68 }], resets: `<span style="color:${G};font-weight:500">Rolling window</span> · refills within 1 min` }) + `<div style="${F}font-size:11px;color:${L}"><span style="font-weight:600;color:${G}">Requests</span> · not reported by provider</div>`)}
${col(colHead(box(CHATGPT(16)), "ChatGPT Plus · Codex", "2 subscriptions") + capacity({ label: "Weekly limit", have: "64%", of: "100%", meta: "avg 64% · 1 fresh · 1 stale excluded", vals: [{ p: 64 }, { p: 58, stale: true }], resets: `<span style="color:${G};font-weight:500">Resets in 6d</span> · Oct 24, 12:50 · Team stale, 18m ago` }) + short(`22% of 100% · 1 fresh · resets in 1h 05m`))}
</div>`;

// ledger: grouped tonal surfaces, alternating row tints, no rules
const COLS = [
  ["Account", 200],
  ["Weekly", 150],
  ["5-hour", 150],
  ["Fable 5 · 7-day", 150],
  ["Reset credits", 130],
  ["", null],
];
const thead = `<div style="display:flex;gap:14px;align-items:center;padding:6px 12px;${F}font-size:10px;font-weight:600;letter-spacing:0.07em;text-transform:uppercase;color:${L}">${COLS.map(([t, w]) => `<div style="${w ? `width:${w}px;flex-shrink:0` : "flex:1"}">${t}</div>`).join("")}</div>`;
const q = (pct, reset, { stale = false, na = false } = {}) =>
  na
    ? `<div style="width:150px;flex-shrink:0;padding-top:2px;${F}font-size:11px;color:${L}">— no model limit reported</div>`
    : `<div style="width:150px;flex-shrink:0;display:flex;flex-direction:column;gap:5px;${F}"><div style="display:flex;justify-content:flex-end"><div style="font-size:13px;font-weight:600;color:${pct == null || stale ? L : pct < 15 ? EM : G}">${pct == null ? "—" : pct + "%"}</div></div>${bar(pct, { stale })}<div style="font-size:11px;color:${stale ? AM : L};white-space:nowrap">${reset}</div></div>`;
const credits = (a, b) =>
  `<div style="width:130px;flex-shrink:0;display:flex;flex-direction:column;gap:2px;${F}font-size:12px;color:${G}"><div>${a}</div><div style="font-size:11px;color:${L}">${b}</div></div>`;
const actions = (resetOn, refreshOn, note, shade) =>
  `<div style="flex:1;display:flex;flex-direction:column;gap:4px;align-items:flex-end"><div style="display:flex;gap:6px">${btn("↺ Reset limits", { disabled: !resetOn, on: shade ? SURF : INSET })}${btn("↻ Refresh quota", { disabled: !refreshOn, on: shade ? SURF : INSET })}</div><div style="${F}font-size:10px;color:${L};white-space:nowrap">${note}</div></div>`;
const who = (label, ident, plan, tags = "") =>
  `<div style="width:200px;flex-shrink:0;display:flex;flex-direction:column;gap:3px;${F}"><div style="font-size:13px;font-weight:600;color:${G}">${label}</div><div style="font-size:11px;color:${L}">${ident} · ${plan}</div>${tags ? `<div style="display:flex;gap:4px;padding-top:2px">${tags}</div>` : ""}</div>`;
const row = (cells, shade) =>
  `<div style="display:flex;gap:14px;align-items:flex-start;padding:12px;border-radius:9px;${shade ? `background:${INSET};` : ""}">${cells.join("")}</div>`;
const group = (mark, name, sub, mode, rows, collapsed = false) =>
  `<div style="display:flex;flex-direction:column;gap:2px;padding:12px 12px ${collapsed ? 12 : 10}px;border-radius:14px;background:${SURF}"><div style="display:flex;align-items:center;gap:8px;padding:2px 12px 4px;${F}"><span style="color:${L};font-size:11px">${collapsed ? "▸" : "▾"}</span>${mark}<div style="font-size:13px;font-weight:600;color:${G}">${name}</div><div style="font-size:12px;color:${L}">${sub}</div>${mode}</div>${collapsed ? "" : thead + rows.join("")}</div>`;
const ledger = `<div layer-name="Ledger" style="display:flex;flex-direction:column;gap:12px;padding:14px 20px 0">
${group(
  box(CLAUDE(15), 16),
  "Claude Max",
  "Anthropic · 3 subscriptions",
  tag("POOLED", EV),
  [
    row(
      [
        who("Personal", "d•••@gmail.com", "Max 20×"),
        q(88, "in 4d · Oct 22, 09:00"),
        q(81, "in 2h 10m · Oct 18, 23:51"),
        q(100, "in 4d · Oct 22, 09:00"),
        credits("1 available", "expires Oct 22, 09:00"),
        actions(true, true, "Provider action · uses 1 credit", true),
      ],
      true,
    ),
    row(
      [
        who("Work", "d•••@acme.co", "Max 20×", tag("● IN USE · 2", G)),
        q(70, "in 5d · Oct 23, 14:20"),
        q(64, "in 3h · Oct 19, 00:41"),
        q(92, "in 5d · Oct 23, 14:20"),
        credits("0 available", "—"),
        actions(false, true, "No reset credits left", false),
      ],
      false,
    ),
    row(
      [
        who("Secondary", "d•••@proton.me", "Max 20×", tag("! BACK IN 38M", EM)),
        q(54, "in 6d · Oct 24, 18:00"),
        q(0, "in 38m · Oct 18, 22:19"),
        q(100, "in 6d · Oct 24, 18:00"),
        credits("Not reported", "none exposed"),
        actions(false, true, "Reset not offered for this account", true),
      ],
      true,
    ),
  ],
)}
${group(box(ANTH(11), 16), "API keys", "Anthropic · 2 keys", tag("POOLED", EV), [], true)}
${group(
  box(CHATGPT(15), 16),
  "ChatGPT Plus · Codex",
  "OpenAI · 2 subscriptions",
  tag("OBSERVED", L),
  [
    row(
      [
        who("Dev", "d•••@acme.co", "Plus", tag("● IN USE · 1", G)),
        q(64, "in 6d · Oct 24, 12:50"),
        q(22, "in 1h 05m · Oct 18, 23:14"),
        q(null, "", { na: true }),
        credits("Not reported", "—"),
        actions(
          false,
          false,
          "Updates on next request · no supported refresh",
          true,
        ),
      ],
      true,
    ),
    row(
      [
        who("Team", "t•••@acme.co", "Plus", tag("◷ STALE", AM)),
        q(null, "— not reported"),
        q(58, "seen 18m ago · not in avg", { stale: true }),
        q(null, "", { na: true }),
        credits("Not reported", "—"),
        actions(
          false,
          false,
          "Updates on next request · no supported refresh",
          false,
        ),
      ],
      false,
    ),
  ],
)}
</div>
<div style="padding:16px 22px 22px;${F}font-size:11px;line-height:17px;color:${L}">Design samples. Capacity adds each account's percentage left (N accounts = N × 100%); segments stay per account and the average sits beside the counts. Resets differ per account, so summaries show the first reset and the range. <b>Reset limits</b> appears only when a provider reports reset credits and runs that provider's own action; it never clears Relay's counters. <b>Refresh quota</b> uses a documented usage read, never a billed request. The OpenAI symbol marks the provider; the ChatGPT logo marks the ChatGPT plan.</div>`;
const window09 = `<div layer-name="Main window — Quotas" style="display:flex;flex-direction:column;width:1240px;background:${CANVAS};border-radius:12px;overflow:hidden;box-shadow:0 22px 50px rgba(28,31,30,0.16)"><div style="display:flex;align-items:center;height:46px;padding:0 14px;background:${CHROME}">${lights}<div style="flex:1;display:flex;justify-content:center">${tabs}</div>${btn("↻ Refresh all", { on: CTRL })}</div>${filters}${search}${summary}${ledger}</div>`;
const html09 = `<div layer-name="Board title" style="display:flex;flex-direction:column;gap:6px;${F}"><div style="font-size:11px;font-weight:500;letter-spacing:0.08em;text-transform:uppercase;color:${EV}">Relay · design review · main window</div><div style="font-size:30px;font-weight:700;letter-spacing:-0.02em;color:${G}">Quotas — full ledger</div><div style="font-size:14px;line-height:21px;color:${L};max-width:900px">Opened by Open Relay. One shared summary surface with a column per comparable group (pooled total, per-account segments, resets, nested model limit), then a grouped ledger with every window, reset, credit and action per account. Separation comes from tonal surfaces and spacing — no borders or gridlines.</div></div>${window09}`;

// ---------- run ----------
const a9 = idOf(
  await tools.paper.create_artboard({
    fileId,
    pageId: PAGE,
    name: "3 · Main window — Quotas ledger",
    styles: {
      width: "1340px",
      height: "fit-content",
      background: "#FFFFFF",
      display: "flex",
      flexDirection: "column",
      gap: "28px",
      padding: "48px",
    },
  }),
);
await tools.paper.write_html({
  fileId,
  targetNodeId: a9,
  mode: "insert-children",
  html: html09,
});
const nodes = {};
nodes[a9] = [{ format: "png", scale: "2x" }];
const ex = await tools.paper.export({
  fileId,
  pageId: PAGE,
  type: "image",
  nodes,
});
return {
  a9,
  path: [...JSON.stringify(ex).matchAll(/filePath\\": \\"([^\\]+)/g)].map(
    (m) => m[1],
  ),
};
