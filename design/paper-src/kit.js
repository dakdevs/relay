// Shared Paper kit for Relay review boards (borderless tonal surfaces, official SVG marks).
// Prepended to each batch file when run through Executor `execute`. Design tooling only — not app code.
const fileId = "01M58Z151ZVWZ4VVK921R3Q5J9";
const F = `font-family:var(--font-ui);font-variant-numeric:tabular-nums;`;
const LT = {
  canvas: "#EEF0EF",
  pop: "#F2F3F2",
  surf: "#FFFFFF",
  inset: "#F4F5F4",
  ctrl: "#E8EBE9",
  track: "#E6E9E7",
  fg: "#1C1F1E",
  mu: "#646A67",
  ev: "#1F6B4F",
  am: "#9A6614",
  em: "#B4432F",
  hatch: "#D3D7D5",
  desk: "#D9DDDB",
  bar: "rgba(255,255,255,0.55)",
  shadow: "0 18px 40px rgba(28,31,30,0.18)",
  anth: "#141413",
  gpt: "#000000",
  onEv: "#FFFFFF",
  lift: "0 1px 2px rgba(28,31,30,0.10)",
  btnTop: "#FFFFFF",
  btnBot: "#F0F2F1",
  btnShadow:
    "0 1px 2px rgba(28,31,30,0.14), inset 0 1px 0 rgba(255,255,255,0.7)",
  btnPress: "inset 0 1px 2px rgba(28,31,30,0.16)",
  evTop: "#2B7D5E",
  evShadow:
    "0 1px 2px rgba(31,107,79,0.38), inset 0 1px 0 rgba(255,255,255,0.18)",
  evPress: "inset 0 1px 2px rgba(0,0,0,0.28)",
  hoverBot: "#E6E9E7",
  focusTop: "#E6F1EB",
  focusBot: "#D3E6DC",
  evHoverBot: "#1A5E45",
  evFocusTop: "#3A9472",
  evFocusBot: "#2B7D5E",
};
const DK = {
  canvas: "#161817",
  pop: "#1B1D1C",
  surf: "#262927",
  inset: "#2E3230",
  ctrl: "#323634",
  track: "#3B3F3D",
  fg: "#ECEEED",
  mu: "#A0A6A3",
  ev: "#4DB389",
  am: "#D9A140",
  em: "#E26A52",
  hatch: "#4A504E",
  desk: "#0F1110",
  bar: "rgba(30,33,32,0.7)",
  shadow: "0 18px 40px rgba(0,0,0,0.55)",
  anth: "#FAF9F5",
  gpt: "#FFFFFF",
  onEv: "#0F1A15",
  lift: "0 1px 2px rgba(0,0,0,0.4)",
  btnTop: "#3A3F3C",
  btnBot: "#313533",
  btnShadow: "0 1px 2px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.06)",
  btnPress: "inset 0 1px 2px rgba(0,0,0,0.5)",
  evTop: "#5CC198",
  evShadow: "0 1px 2px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,255,255,0.2)",
  evPress: "inset 0 1px 2px rgba(0,0,0,0.35)",
  hoverBot: "#2D312F",
  focusTop: "#3E5A4D",
  focusBot: "#34493F",
  evHoverBot: "#45A57D",
  evFocusTop: "#86D9B6",
  evFocusBot: "#5CC198",
};
const idOf = (res, name) => {
  const s = JSON.stringify(res);
  if (!name) return s.match(/\\"id\\": \\"([0-9A-Z]+-[0-9]+)\\"/)[1];
  const re = new RegExp(
    `\\\\"id\\\\": \\\\"([0-9A-Z]+-[0-9]+)\\\\",\\\\n\\s*\\\\"name\\\\": \\\\"${name}\\\\"`,
  );
  return s.match(re)[1];
};
const exportPaths = (r) =>
  [...JSON.stringify(r).matchAll(/filePath\\": \\"([^\\]+)/g)].map((m) => m[1]);
// Official marks — see design/assets/logo-sources.md
const ANTH = (h, c) =>
  `<svg width="${Math.round((h * 92) / 64)}" height="${h}" viewBox="0 0 92 64" style="flex-shrink:0"><path d="M65.92 0H51.9314L77.44 64H91.4286L65.92 0Z" fill="${c}"/><path d="M25.5086 0L0 64H14.2629L19.4798 50.56H46.166L51.3829 64H65.6457L40.1371 0H25.5086ZM24.0933 38.6743L32.8229 16.1829L41.5525 38.6743H24.0933Z" fill="${c}"/></svg>`;
const CLAUDE = (s) =>
  `<svg width="${s}" height="${s}" viewBox="0 0 94 94" style="flex-shrink:0"><path d="M18.7657 62.4437L37.1822 52.1167L37.4857 51.2122L37.1822 50.7085H36.2715L33.1852 50.5208L22.6615 50.2391L13.5545 49.8636L4.70044 49.3942L2.47428 48.9248L0.399902 46.1553L0.602281 44.794L2.47428 43.5266L5.15579 43.7613L11.0754 44.1837L19.98 44.794L26.4055 45.1695L35.9679 46.1553H37.4857L37.6881 45.545L37.1822 45.1695L36.7774 44.794L27.5692 38.5508L17.6021 31.9791L12.3908 28.1769L9.60812 26.2524L8.19147 24.4686L7.58433 20.5256L10.1141 17.7091L13.5545 17.9438L14.4146 18.1785L17.9056 20.8542L25.343 26.6279L35.0572 33.7629L36.4739 34.9364L37.0443 34.5514L37.1316 34.2792L36.4739 33.1996L31.212 23.6706L25.596 13.9539L23.0663 9.91695L22.4086 7.52296C22.1538 6.51831 22.0038 5.68714 22.0038 4.65957L24.8877 0.716544L26.5067 0.200195L30.4025 0.716544L32.0215 2.12477L34.4501 7.66379L38.3458 16.3478L44.4172 28.1769L46.188 31.6975L47.1493 34.9364L47.5035 35.9222H48.1106V35.3589L48.6166 28.6933L49.5273 20.5256L50.438 10.0108L50.7415 7.05356L52.2088 3.48605L55.1433 1.56148L57.42 2.64112L59.292 5.31674L59.039 7.05356L57.926 14.2824L55.7504 25.5952L54.3337 33.1996H55.1433L56.1046 32.2138L59.9497 27.1442L66.3752 19.0704L69.2085 15.8784L72.5478 12.3579L74.6728 10.668H78.7203L81.6548 15.0804L80.3394 19.6337L76.1906 24.8911L72.7502 29.3504L67.8172 35.9595L64.7562 41.2734L65.0307 41.7118L65.7681 41.6489L76.8989 39.255L82.9197 38.1753L90.1041 36.9549L93.3422 38.457L93.6963 40.006L92.4315 43.151L84.7411 45.0287L75.7353 46.8594L62.3244 50.0164L62.1759 50.1358L62.3512 50.3958L68.399 50.9432L70.9794 51.084H77.3037L89.0922 51.9759L92.1785 53.9944L93.9999 56.4822L93.6963 58.4068L88.9404 60.8008L82.5655 59.2987L67.6401 55.7312L62.5301 54.4638H61.8217V54.8862L66.0717 59.064L73.9139 66.1051L83.6786 75.2116L84.1845 77.4648L82.9197 79.2485L81.6042 79.0608L73.0032 72.5829L69.6639 69.6726L62.1759 63.3356H61.67V63.9928L63.3902 66.5276L72.5478 80.2812L73.0032 84.5059L72.3454 85.8672L69.9675 86.7121L67.3871 86.2427L61.9735 78.6852L56.4587 70.2359L52.0064 62.6315L51.4687 62.971L48.8189 91.2654L47.6047 92.7206L44.7714 93.8002L42.3934 92.0164L41.1286 89.1061L42.3934 83.3324L43.9113 75.8219L45.1255 69.8604L46.2386 62.4437L46.9184 59.9661L46.8583 59.8003L46.3153 59.8916L40.7238 67.5603L32.2239 79.0608L25.4948 86.2427L23.8758 86.8999L21.0931 85.4447L21.3461 82.863L22.9145 80.5629L32.2239 68.7338L37.8399 61.3641L41.4594 57.1337L41.4242 56.5218L41.2244 56.5048L16.489 72.6299L12.0873 73.1932L10.1647 71.4094L10.4176 68.4991L11.3283 67.5603L18.7657 62.4437Z" fill="#D97757"/></svg>`;
const CHATGPT = (s, c) =>
  `<svg width="${s}" height="${s}" viewBox="0 0 21 21" style="flex-shrink:0"><path d="M11.8102 19.1625C11.2327 19.1625 10.6841 19.0528 10.1643 18.8334C9.64459 18.6139 9.18259 18.3078 8.77834 17.9151C8.33944 18.0653 7.88322 18.1404 7.40967 18.1404C6.63582 18.1404 5.91972 17.9498 5.26137 17.5686C4.60302 17.1875 4.07172 16.6677 3.66747 16.0094C3.27477 15.351 3.07842 14.6176 3.07842 13.8091C3.07842 13.4742 3.12462 13.1103 3.21702 12.7176C2.75502 12.2903 2.39697 11.7994 2.14287 11.245C1.88877 10.679 1.76172 10.09 1.76172 9.47785C1.76172 8.85415 1.89454 8.25355 2.16019 7.67605C2.42584 7.09855 2.79544 6.6019 3.26899 6.1861C3.75409 5.75875 4.31427 5.46422 4.94952 5.30252C5.07657 4.64417 5.34222 4.05512 5.74647 3.53537C6.16227 3.00407 6.67047 2.58827 7.27107 2.28797C7.87167 1.98767 8.51269 1.83752 9.19414 1.83752C9.77164 1.83752 10.3203 1.94725 10.84 2.1667C11.3598 2.38615 11.8218 2.69222 12.226 3.08492C12.6649 2.93477 13.1211 2.8597 13.5947 2.8597C14.3685 2.8597 15.0846 3.05027 15.743 3.43142C16.4013 3.81257 16.9269 4.33232 17.3196 4.99067C17.7238 5.64902 17.9259 6.38245 17.9259 7.19095C17.9259 7.5259 17.8797 7.88973 17.7873 8.28242C18.2493 8.70977 18.6074 9.20643 18.8615 9.77238C19.1156 10.3268 19.2426 10.91 19.2426 11.5222C19.2426 12.1459 19.1098 12.7465 18.8442 13.324C18.5785 13.9015 18.2031 14.4039 17.718 14.8313C17.2445 15.2471 16.6901 15.5358 16.0548 15.6975C15.9278 16.3559 15.6564 16.9449 15.2406 17.4647C14.8363 17.996 14.3339 18.4118 13.7333 18.7121C13.1327 19.0124 12.4917 19.1625 11.8102 19.1625ZM7.53094 16.9969C8.10844 16.9969 8.61087 16.8756 9.03822 16.6331L12.2953 14.762C12.4108 14.6811 12.4686 14.5714 12.4686 14.4328V12.9429L8.27592 15.351C8.02182 15.5012 7.76772 15.5012 7.51362 15.351L4.23919 13.4626C4.23919 13.4973 4.23342 13.5377 4.22187 13.5839C4.22187 13.6301 4.22187 13.6994 4.22187 13.7918C4.22187 14.3808 4.36047 14.9237 4.63767 15.4203C4.92642 15.9054 5.32489 16.2866 5.83309 16.5638C6.34129 16.8525 6.90724 16.9969 7.53094 16.9969ZM7.70419 14.1729C7.77349 14.2076 7.83702 14.2249 7.89477 14.2249C7.95252 14.2249 8.01027 14.2076 8.06802 14.1729L9.36739 13.4279L5.19207 11.0024C4.93797 10.8523 4.81092 10.6271 4.81092 10.3268V6.56725C4.23342 6.82135 3.77142 7.21405 3.42492 7.74535C3.07842 8.2651 2.90517 8.8426 2.90517 9.47785C2.90517 10.0438 3.04954 10.5866 3.33829 11.1064C3.62704 11.6261 4.00242 12.0189 4.46442 12.2845L7.70419 14.1729ZM11.8102 18.0191C12.4224 18.0191 12.9768 17.8805 13.4734 17.6033C13.9701 17.3261 14.3628 16.9449 14.6515 16.4598C14.9403 15.9747 15.0846 15.4319 15.0846 14.8313V11.0891C15.0846 10.9505 15.0269 10.8465 14.9114 10.7772L13.5947 10.0149V14.8486C13.5947 15.1489 13.4676 15.3741 13.2135 15.5243L9.93912 17.4127C10.5051 17.817 11.1288 18.0191 11.8102 18.0191ZM12.4686 11.6781V9.32192L10.5108 8.21312L8.53579 9.32192V11.6781L10.5108 12.7869L12.4686 11.6781ZM7.40967 6.15145C7.40967 5.85115 7.53672 5.62592 7.79082 5.47577L11.0652 3.58735C10.4993 3.1831 9.87559 2.98097 9.19414 2.98097C8.58199 2.98097 8.02759 3.11957 7.53094 3.39677C7.03429 3.67397 6.64159 4.05512 6.35284 4.54022C6.07564 5.02532 5.93704 5.56817 5.93704 6.16878V9.89365C5.93704 10.0323 5.99479 10.142 6.11029 10.2228L7.40967 10.9851V6.15145ZM16.2108 14.4328C16.7883 14.1787 17.2445 13.786 17.5794 13.2547C17.9259 12.7234 18.0992 12.1459 18.0992 11.5222C18.0992 10.9562 17.9548 10.4134 17.6661 9.89365C17.3773 9.3739 17.0019 8.9812 16.5399 8.71555L13.3002 6.84445C13.2309 6.79825 13.1673 6.78092 13.1096 6.79248C13.0518 6.79248 12.9941 6.8098 12.9363 6.84445L11.637 7.5721L15.8296 10.0149C15.9567 10.0842 16.0491 10.1766 16.1068 10.2921C16.1761 10.3961 16.2108 10.5231 16.2108 10.6733V14.4328ZM12.7284 5.6317C12.9825 5.47 13.2366 5.47 13.4907 5.6317L16.7825 7.55477C16.7825 7.47393 16.7825 7.36997 16.7825 7.24292C16.7825 6.68852 16.6439 6.163 16.3667 5.66635C16.101 5.15815 15.7141 4.7539 15.2059 4.4536C14.7093 4.1533 14.1318 4.00315 13.4734 4.00315C12.8959 4.00315 12.3935 4.12442 11.9661 4.36697L8.70904 6.23808C8.59354 6.31892 8.53579 6.42865 8.53579 6.56725V8.0572L12.7284 5.6317Z" fill="${c}"/></svg>`;
const OPENAI = (s, c) =>
  `<svg width="${s}" height="${s}" viewBox="0 0 32 32" style="flex-shrink:0"><path d="M29.71,13.09A8.09,8.09,0,0,0,20.34,2.68a8.08,8.08,0,0,0-13.7,2.9A8.08,8.08,0,0,0,2.3,18.9,8,8,0,0,0,3,25.45a8.08,8.08,0,0,0,8.69,3.87,8,8,0,0,0,6,2.68,8.09,8.09,0,0,0,7.7-5.61,8,8,0,0,0,5.33-3.86A8.09,8.09,0,0,0,29.71,13.09Zm-12,16.82a6,6,0,0,1-3.84-1.39l.19-.11,6.37-3.68a1,1,0,0,0,.53-.91v-9l2.69,1.56a.08.08,0,0,1,.05.07v7.44A6,6,0,0,1,17.68,29.91ZM4.8,24.41a6,6,0,0,1-.71-4l.19.11,6.37,3.68a1,1,0,0,0,1,0l7.79-4.49V22.8a.09.09,0,0,1,0,.08L13,26.6A6,6,0,0,1,4.8,24.41ZM3.12,10.53A6,6,0,0,1,6.28,7.9v7.57a1,1,0,0,0,.51.9l7.75,4.47L11.85,22.4a.14.14,0,0,1-.09,0L5.32,18.68a6,6,0,0,1-2.2-8.18Zm22.13,5.14-7.78-4.52L20.16,9.6a.08.08,0,0,1,.09,0l6.44,3.72a6,6,0,0,1-.9,10.81V16.56A1.06,1.06,0,0,0,25.25,15.67Zm2.68-4-.19-.12-6.36-3.7a1,1,0,0,0-1.05,0l-7.78,4.49V9.2a.09.09,0,0,1,0-.09L19,5.4a6,6,0,0,1,8.91,6.21ZM11.08,17.15,8.38,15.6a.14.14,0,0,1-.05-.08V8.1a6,6,0,0,1,9.84-4.61L18,3.6,11.61,7.28a1,1,0,0,0-.53.91ZM12.54,14,16,12l3.47,2v4L16,20l-3.47-2Z" fill="${c}"/></svg>`;
const box = (svg, sz = 18) =>
  `<div style="width:${sz}px;height:${sz}px;flex-shrink:0;display:flex;align-items:center;justify-content:center">${svg}</div>`;
// Sample data (design only): calendar week Mon–Sun, today Sun Oct 18 09:41 local; month Oct 1–18 vs Sep 1–18.
const WEEK = {
  cur: [1.1, 1.4, 0.9, 1.6, 1.3, 0.8, 0.3],
  prior: [1.0, 1.2, 1.1, 1.2, 1.0, 0.6, 0.2],
  n: 7,
  curLabel: "This week",
  priorLabel: "Last week",
  span: "same span last week",
};
const MONTH = {
  cur: [
    1.2, 1.3, 0.9, 0.4, 0.3, 1.4, 1.5, 1.2, 1.3, 1.1, 0.5, 0.3, 1.6, 1.4, 1.2,
    1.6, 1.3, 0.3,
  ],
  prior: [
    1.0, 1.1, 1.2, 0.9, 0.3, 0.2, 1.2, 1.3, 1.0, 1.1, 1.2, 0.4, 0.2, 1.3, 1.2,
    1.1, 1.0, 0.2,
  ],
  n: 31,
  curLabel: "October",
  priorLabel: "September",
  span: "Oct 1–18 vs Sep 1–18",
};
const sum = (a) => a.reduce((s, v) => s + (v || 0), 0);
const fmtM = (v) => `${v.toFixed(1)}M`;

const kit = (T) => {
  const level = (p) => (p < 15 ? T.em : p < 40 ? T.am : T.ev);
  const chev = (o) =>
    `<svg width="10" height="10" viewBox="0 0 10 10" style="flex-shrink:0;transform:rotate(${o ? 90 : 0}deg)"><path d="M3.5 2 L7 5 L3.5 8" fill="none" stroke="${T.mu}" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
  const tag = (t, c) =>
    `<div style="flex-shrink:0;width:fit-content;padding:2px 7px;border-radius:5px;background:color-mix(in srgb, ${c} 14%, transparent);${F}font-size:9px;font-weight:600;letter-spacing:0.06em;color:${c};white-space:nowrap">${t}</div>`;
  const hatchBg = `background-image:repeating-linear-gradient(135deg,${T.hatch} 0px,${T.hatch} 2px,transparent 2px,transparent 5px);background-color:${T.track}`;
  const barEl = (pct, stale = false, h = 4) =>
    pct == null
      ? `<div style="flex:1;height:${h}px;border-radius:3px;${hatchBg}"></div>`
      : `<div style="flex:1;height:${h}px;border-radius:3px;background:${T.track};overflow:hidden;display:flex"><div style="width:${pct}%;height:${h}px;border-radius:3px;background:${level(pct)};opacity:${stale ? 0.35 : 1}"></div></div>`;
  const win = ({
    label,
    pct,
    meta,
    stale = false,
    metaColor = T.mu,
    indent = 18,
  }) => {
    const vcol = pct == null || stale ? T.mu : pct < 15 ? T.em : T.fg;
    return `<div style="display:flex;align-items:center;gap:10px;padding-left:${indent}px;${F}"><div style="width:62px;flex-shrink:0;font-size:10px;font-weight:500;letter-spacing:0.05em;color:${T.mu};text-transform:uppercase;white-space:nowrap">${label}</div>${barEl(pct, stale)}<div style="width:38px;flex-shrink:0;text-align:right;font-size:13px;font-weight:500;color:${vcol}">${pct == null ? "—" : pct + "%"}</div><div style="width:80px;flex-shrink:0;text-align:right;font-size:11px;color:${metaColor};white-space:nowrap">${meta}</div></div>`;
  };
  const head = ({
    mark = "",
    name,
    count,
    mode,
    open = false,
    note = "",
    noteColor = T.mu,
    focus = false,
  }) =>
    `<div style="display:flex;align-items:center;gap:8px;min-height:26px;margin:0 -6px;padding:0 6px;border-radius:7px;${focus ? `background:color-mix(in srgb, ${T.ev} 14%, transparent);` : ""}${F}">${chev(open)}${mark}<div style="font-size:13px;font-weight:600;color:${T.fg};white-space:nowrap">${name}</div><div style="font-size:11px;color:${T.mu};white-space:nowrap">${count}</div>${mode === "pooled" ? tag("POOLED", T.ev) : tag("OBSERVED", T.mu)}<div style="flex:1"></div><div style="font-size:11px;font-weight:500;color:${noteColor};white-space:nowrap">${note}</div></div>`;
  const inUse = (name, n) =>
    `<div style="display:flex;align-items:center;gap:5px;padding-left:18px;${F}font-size:11px;color:${T.mu}"><span style="color:${T.fg}">●</span><span style="color:${T.fg};font-weight:600">${name}</span><span>${n} in use</span></div>`;
  const acct = ({
    label,
    ident,
    seen,
    seenColor = T.mu,
    tags = "",
    rows,
    note = "",
    focus = false,
  }) =>
    `<div style="display:flex;flex-direction:column;gap:7px;padding:10px;margin-left:18px;border-radius:9px;background:${focus ? `color-mix(in srgb, ${T.ev} 12%, ${T.inset})` : T.inset}"><div style="display:flex;align-items:flex-start;gap:6px;${F}"><div style="display:flex;flex-direction:column;gap:1px;flex:1"><div style="font-size:12px;font-weight:600;color:${T.fg}">${label}</div><div style="font-size:11px;color:${T.mu};white-space:nowrap">${ident} · <span style="color:${seenColor}">${seen}</span></div></div><div style="display:flex;gap:4px;padding-top:1px">${tags}</div></div>${rows.map((r) => win({ ...r, indent: 0 })).join("")}${note ? `<div style="${F}font-size:11px;line-height:16px;color:${T.mu}">${note}</div>` : ""}</div>`;
  const section = (mark, name, inner) =>
    `<div style="display:flex;flex-direction:column;gap:12px;padding:10px 12px 12px;border-radius:12px;background:${T.surf}"><div style="display:flex;align-items:center;gap:7px;${F}">${mark}<div style="font-size:11px;font-weight:600;letter-spacing:0.02em;color:${T.mu}">${name}</div></div>${inner}</div>`;
  const group = (h, rows) =>
    `<div style="display:flex;flex-direction:column;gap:8px">${h}${rows.join("")}</div>`;
  const header = `<div layer-name="Header" style="display:flex;align-items:center;gap:8px;padding:6px 8px 2px;${F}"><div style="font-size:15px;font-weight:600;color:${T.fg};letter-spacing:-0.01em">Relay</div><div style="display:flex;align-items:center;gap:5px;padding:2px 8px 2px 6px;border-radius:10px;background:color-mix(in srgb, ${T.ev} 14%, transparent)"><div style="width:6px;height:6px;border-radius:3px;background:${T.ev}"></div><div style="font-size:11px;font-weight:500;color:${T.ev}">Running</div></div><div style="flex:1"></div><div style="font-size:11px;color:${T.mu}">127.0.0.1:8317</div></div>`;
  // Sparkline: current (solid) vs prior equivalent period (dashed). null = no data (gap), 0 = real zero.
  const spark = ({
    w = 140,
    h = 34,
    d = WEEK,
    cur = d.cur,
    prior = d.prior,
    partial = true,
    priorFrom = 0,
    gapAt = [],
    zeroAt = [],
    marker = null,
  }) => {
    const all = [...cur, ...(prior || [])].filter((v) => v != null);
    const max = Math.max(0.5, ...all) * 1.12;
    const x = (i) => 3 + (i * (w - 6)) / (d.n - 1);
    const y = (v) => h - 3 - (v / max) * (h - 6);
    const runs = (arr, from = 0) => {
      const out = [];
      let cur2 = [];
      arr.forEach((v, i) => {
        if (i < from || v == null) {
          if (cur2.length) out.push(cur2);
          cur2 = [];
        } else cur2.push(`${x(i).toFixed(1)},${y(v).toFixed(1)}`);
      });
      if (cur2.length) out.push(cur2);
      return out;
    };
    const bands = gapAt
      .map(
        (i) =>
          `<rect x="${(x(i) - (w - 6) / (d.n - 1) / 2).toFixed(1)}" y="2" width="${((w - 6) / (d.n - 1)).toFixed(1)}" height="${h - 4}" rx="2" fill="${T.track}"/>`,
      )
      .join("");
    const pr = prior
      ? runs(prior, priorFrom)
          .map(
            (r) =>
              `<polyline points="${r.join(" ")}" fill="none" stroke="${T.mu}" stroke-opacity="0.55" stroke-width="1.25" stroke-dasharray="2.5 2.5" stroke-linecap="round"/>`,
          )
          .join("")
      : "";
    const cr = runs(cur)
      .map(
        (r) =>
          `<polyline points="${r.join(" ")}" fill="none" stroke="${T.ev}" stroke-width="1.75" stroke-linejoin="round" stroke-linecap="round"/>`,
      )
      .join("");
    const zeros = zeroAt
      .map(
        (i) =>
          `<circle cx="${x(i).toFixed(1)}" cy="${y(0).toFixed(1)}" r="2" fill="${T.ev}"/>`,
      )
      .join("");
    const li = cur.length - 1;
    const end =
      cur[li] != null
        ? `<circle cx="${x(li).toFixed(1)}" cy="${y(cur[li]).toFixed(1)}" r="2.6" fill="${partial ? T.surf : T.ev}" stroke="${T.ev}" stroke-width="1.5"/>`
        : "";
    const mk =
      marker != null
        ? `<line x1="${x(marker).toFixed(1)}" y1="1" x2="${x(marker).toFixed(1)}" y2="${h - 1}" stroke="${T.mu}" stroke-opacity="0.35" stroke-width="1"/><circle cx="${x(marker).toFixed(1)}" cy="${y(cur[marker]).toFixed(1)}" r="2.6" fill="${T.ev}"/>`
        : "";
    return `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" style="flex-shrink:0">${bands}<line x1="3" y1="${h - 3}" x2="${w - 3}" y2="${h - 3}" stroke="${T.track}" stroke-width="1"/>${pr}${cr}${zeros}${mk}${end}</svg>`;
  };
  const seg = (sel) =>
    `<div style="display:flex;padding:2px;border-radius:7px;background:${T.inset}">${["Week", "Month"].map((t) => `<div style="padding:2px 9px;border-radius:5px;${t === sel ? `background:${T.surf};box-shadow:${T.lift};color:${T.fg};font-weight:600;` : `color:${T.mu};font-weight:500;`}${F}font-size:11px">${t}</div>`).join("")}</div>`;
  const delta = (c, p) => {
    const v = Math.round(((c - p) / p) * 100);
    return tag(`${v >= 0 ? "↑" : "↓"} ${Math.abs(v)}%`, v >= 0 ? T.ev : T.am);
  };
  // Tactile button family: rest = soft luminance shift + short elevation; pressed = sinks 1px, inset shade; disabled = flat, quiet.
  const surfaceFor = (kind, state) => {
    if (state === "disabled") return `background:${T.inset};opacity:0.45;`;
    const p = kind === "primary";
    const top = p ? T.evTop : T.btnTop,
      bot = p ? T.ev : T.btnBot;
    if (state === "pressed")
      return `background:linear-gradient(180deg, ${bot}, ${bot});box-shadow:${p ? T.evPress : T.btnPress};transform:translateY(1px);`;
    if (state === "focus")
      return `background:linear-gradient(180deg, ${p ? T.evFocusTop : T.focusTop}, ${p ? T.evFocusBot : T.focusBot});box-shadow:${p ? T.evShadow : T.btnShadow};`;
    if (state === "hover")
      return `background:linear-gradient(180deg, ${p ? T.evTop : T.btnTop}, ${p ? T.evHoverBot : T.hoverBot});box-shadow:${p ? T.evShadow : T.btnShadow};`;
    return `background:linear-gradient(180deg, ${top}, ${bot});box-shadow:${p ? T.evShadow : T.btnShadow};`;
  };
  const button = (
    label,
    { kind = "secondary", state = "rest", icon = "", h = 28 } = {},
  ) =>
    `<div style="display:flex;align-items:center;justify-content:center;gap:6px;height:${h}px;${label ? "padding:0 12px;" : `width:${h}px;`}border-radius:7px;flex-shrink:0;${surfaceFor(kind, state)}${F}font-size:12px;font-weight:${kind === "primary" ? 600 : 500};color:${kind === "primary" && state !== "disabled" ? T.onEv : T.fg};white-space:nowrap">${icon}${label}</div>`;
  const iconBtn = (svg, state = "rest") => button("", { icon: svg, state });
  const pauseSvg = `<svg width="12" height="12" viewBox="0 0 12 12"><rect x="3" y="2.5" width="2" height="7" rx="0.8" fill="${T.mu}"/><rect x="7" y="2.5" width="2" height="7" rx="0.8" fill="${T.mu}"/></svg>`;
  const gearSvg = `<svg width="14" height="14" viewBox="0 0 14 14"><circle cx="7" cy="7" r="2.2" fill="none" stroke="${T.mu}" stroke-width="1.3"/><path d="M7 1.5v1.6M7 10.9v1.6M1.5 7h1.6M10.9 7h1.6M3.1 3.1l1.1 1.1M9.8 9.8l1.1 1.1M3.1 10.9l1.1-1.1M9.8 4.2l1.1-1.1" stroke="${T.mu}" stroke-width="1.3" stroke-linecap="round"/></svg>`;
  const openRelay = `<div layer-name="Open Relay" style="display:flex">${button(`Open Relay<span style="font-weight:500;opacity:0.85">↗</span>`, { kind: "primary" })}</div>`;
  const through = (mode = "Week", opts = {}) => {
    const d = mode === "Week" ? WEEK : MONTH;
    const c = sum(d.cur),
      p = sum(d.prior);
    return `<div layer-name="Through Relay" style="display:flex;flex-direction:column;gap:6px;${F}"><div style="display:flex;align-items:center;gap:8px"><div style="font-size:11px;font-weight:600;color:${T.fg}">Through Relay</div><div style="font-size:11px;color:${T.mu}">tokens</div><div style="flex:1"></div>${seg(mode)}</div><div style="display:flex;align-items:flex-end;gap:10px"><div style="display:flex;flex-direction:column;gap:3px;flex:1;min-width:0"><div style="display:flex;align-items:baseline;gap:7px"><div style="font-size:19px;font-weight:700;letter-spacing:-0.02em;color:${T.fg}">${fmtM(c)}</div>${delta(c, p)}</div><div style="font-size:10px;line-height:14px;color:${T.mu}">vs ${fmtM(p)} · ${d.span}</div></div>${spark({ d, w: 120, ...opts })}</div><div style="display:flex;align-items:center;gap:12px;font-size:10px;color:${T.mu}"><div style="display:flex;align-items:center;gap:4px"><div style="width:12px;height:2px;border-radius:1px;background:${T.ev}"></div>${d.curLabel} (to now)</div><div style="display:flex;align-items:center;gap:4px"><svg width="12" height="2" style="flex-shrink:0"><line x1="0" y1="1" x2="12" y2="1" stroke="${T.mu}" stroke-opacity="0.7" stroke-width="1.5" stroke-dasharray="2.5 2"/></svg>${d.priorLabel}, same span</div></div></div>`;
  };
  const footer = (mode = "Week") =>
    `<div layer-name="Footer (pinned)" style="display:flex;flex-direction:column;gap:10px;padding:10px 12px;border-radius:12px;background:${T.surf}">${through(mode)}<div style="display:flex;align-items:center;gap:6px">${iconBtn(pauseSvg)}${iconBtn(gearSvg)}<div style="flex:1"></div>${openRelay}</div></div>`;
  const pop = (name, inner, mode = "Week") =>
    `<div layer-name="${name}" style="display:flex;flex-direction:column;gap:8px;width:360px;margin:6px 16px 0 auto;padding:8px;background:${T.pop};border-radius:16px;box-shadow:${T.shadow};overflow:hidden">${header}${inner}${footer(mode)}</div>`;
  const trayIcon = (c) =>
    `<svg width="16" height="16" viewBox="0 0 16 16"><circle cx="3.5" cy="8" r="1.8" fill="${c}"/><path d="M5.5 8 H8 M8 8 C10 8 10 4 12.5 4 M8 8 C10 8 10 12 12.5 12" fill="none" stroke="${c}" stroke-width="1.4" stroke-linecap="round"/><circle cx="13" cy="4" r="1.3" fill="${c}"/><circle cx="13" cy="12" r="1.3" fill="${c}"/></svg>`;
  const menubar = (t) =>
    `<div style="display:flex;align-items:center;justify-content:flex-end;gap:14px;height:26px;padding:0 14px;background:${T.bar};${F}font-size:13px;color:${T.fg}"><div style="display:flex;align-items:center;justify-content:center;width:28px;height:22px;border-radius:5px;background:color-mix(in srgb, ${T.fg} 14%, transparent)">${trayIcon(T.fg)}</div><div>${t}</div></div>`;
  const INUSE = (n) => tag(`● IN USE · ${n}`, T.fg),
    EXH = tag("! EXHAUSTED", T.em),
    STALE = tag("◷ STALE", T.am);
  const claude = (open, focusAcct = false) =>
    group(
      head({
        mark: box(CLAUDE(14), 16),
        name: "Claude Max",
        count: "3 subscriptions",
        mode: "pooled",
        open,
      }),
      [
        win({ label: "5-hour", pct: 48, meta: "avg · 3 fresh" }),
        win({ label: "Weekly", pct: 71, meta: "avg · 3 fresh" }),
        ...(open
          ? [
              win({ label: "Fable 7d", pct: 97, meta: "avg · 3 fresh" }),
              acct({
                label: "Personal",
                ident: "d•••@gmail.com",
                seen: "seen 2m ago",
                rows: [
                  { label: "5-hour", pct: 81, meta: "resets 2h 10m" },
                  { label: "Weekly", pct: 88, meta: "resets 4d" },
                  { label: "Fable 7d", pct: 100, meta: "resets 4d" },
                ],
                note: "Reset credits: 1 · expire Oct 22, 09:00",
              }),
              acct({
                label: "Work",
                ident: "d•••@acme.co",
                seen: "seen 4m ago",
                tags: INUSE(2),
                focus: focusAcct,
                rows: [
                  { label: "5-hour", pct: 64, meta: "resets 3h" },
                  { label: "Weekly", pct: 70, meta: "resets 5d" },
                  { label: "Fable 7d", pct: 92, meta: "resets 5d" },
                ],
              }),
              acct({
                label: "Secondary",
                ident: "d•••@proton.me",
                seen: "seen 1m ago",
                tags: EXH,
                rows: [
                  {
                    label: "5-hour",
                    pct: 0,
                    meta: "resets 38m",
                    metaColor: T.em,
                  },
                  { label: "Weekly", pct: 54, meta: "resets 6d" },
                  { label: "Fable 7d", pct: 100, meta: "resets 6d" },
                ],
                note: "Back in 38m.",
              }),
            ]
          : [inUse("Work", 2)]),
      ],
    );
  const apiKeys = (open) =>
    group(head({ name: "API keys", count: "2 keys", mode: "pooled", open }), [
      win({ label: "Tokens/min", pct: 76, meta: "avg · 2 fresh" }),
    ]);
  const codex = (open, focusHead = false) =>
    group(
      head({
        mark: box(CHATGPT(14, T.gpt), 16),
        name: "ChatGPT Plus · Codex",
        count: "2",
        mode: "observed",
        open,
        note: "1 stale",
        noteColor: T.am,
        focus: focusHead,
      }),
      [
        win({ label: "5-hour", pct: 22, meta: "avg · 1 fresh" }),
        win({ label: "Weekly", pct: 64, meta: "avg · 1 fresh" }),
        ...(open
          ? [
              acct({
                label: "Dev",
                ident: "d•••@acme.co",
                seen: "seen 3m ago",
                tags: INUSE(1),
                rows: [
                  { label: "5-hour", pct: 22, meta: "resets 1h 05m" },
                  { label: "Weekly", pct: 64, meta: "resets 6d" },
                ],
              }),
              acct({
                label: "Team",
                ident: "t•••@acme.co",
                seen: "seen 18m ago",
                seenColor: T.am,
                tags: STALE,
                rows: [
                  { label: "5-hour", pct: 58, stale: true, meta: "not in avg" },
                  { label: "Weekly", pct: null, meta: "not reported" },
                ],
                note: "Updates on this instance's next request.",
              }),
            ]
          : [inUse("Dev", 1)]),
      ],
    );
  const anthSection = (open, focusAcct) =>
    section(
      box(ANTH(10, T.anth), 14),
      "Anthropic",
      claude(open, focusAcct) + apiKeys(false),
    );
  const oaiSection = (open) =>
    section(box(OPENAI(13, T.gpt), 14), "OpenAI", codex(open));
  return {
    T,
    button,
    surfaceFor,
    tag,
    barEl,
    win,
    head,
    acct,
    section,
    spark,
    seg,
    through,
    footer,
    pop,
    trayIcon,
    menubar,
    anthSection,
    oaiSection,
    iconBtn,
    pauseSvg,
    gearSvg,
    openRelay,
    INUSE,
    EXH,
    STALE,
    delta,
  };
};

// board chrome (boards sit on a white page; windows and popovers carry their own surfaces)
const G = "#1C1F1E",
  L = "#646A67",
  EV = "#1F6B4F",
  AM = "#9A6614",
  EM = "#B4432F";
const title = (eyebrow, h, body) =>
  `<div layer-name="Board title" style="display:flex;flex-direction:column;gap:6px;${F}"><div style="font-size:11px;font-weight:500;letter-spacing:0.08em;text-transform:uppercase;color:${EV}">${eyebrow}</div><div style="font-size:30px;font-weight:700;letter-spacing:-0.02em;color:${G}">${h}</div><div style="font-size:14px;line-height:21px;color:${L};max-width:880px">${body}</div></div>`;
const cap = (h, b) =>
  `<div style="display:flex;flex-direction:column;gap:4px;${F}"><div style="font-size:13px;font-weight:600;color:${G}">${h}</div><div style="font-size:12px;line-height:18px;color:${L}">${b}</div></div>`;
const board = (w) => ({
  width: `${w}px`,
  height: "fit-content",
  background: "#FFFFFF",
  display: "flex",
  flexDirection: "column",
  gap: "32px",
  padding: "48px",
});
const Lk = kit(LT),
  Dk = kit(DK);
const scene = (K, inner, w = 460) =>
  `<div style="display:flex;flex-direction:column;width:${w}px;flex-shrink:0;background:${K.T.desk};border-radius:12px;overflow:hidden;padding-bottom:28px">${K.menubar("Sun 18 Oct  9:41")}${inner}</div>`;
