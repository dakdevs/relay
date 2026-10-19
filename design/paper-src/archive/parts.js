// HTML fragment builders for the Relay Paper boards. Pasted into Executor `execute` programs.
const C = (n) => `var(--color-${n})`;
const F = `font-family:var(--font-ui);font-variant-numeric:tabular-nums;`;
const level = (p) =>
  p == null ? null : p < 15 ? "ember" : p < 40 ? "amber" : "evergreen";
const chevron = (open) =>
  `<svg width="10" height="10" viewBox="0 0 10 10" style="flex-shrink:0;transform:rotate(${open ? 90 : 0}deg)"><path d="M3.5 2 L7 5 L3.5 8" fill="none" stroke="${C("lichen")}" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const hatch = `background-image:repeating-linear-gradient(135deg, ${C("mist")} 0px, ${C("mist")} 2px, transparent 2px, transparent 5px);background-color:${C("frost")};`;

// One quota window line: label | bar | value | meta. Fixed lanes so rows align.
// pct: remaining percent or null (unknown). stale: dims the fill.
const win = ({
  label,
  pct,
  meta,
  stale = false,
  metaColor = "lichen",
  indent = 18,
}) => {
  const lv = level(pct);
  const track =
    pct == null
      ? `<div style="flex:1;height:4px;border-radius:var(--radius-bar);${hatch}"></div>`
      : `<div style="flex:1;height:4px;border-radius:var(--radius-bar);background:${C("frost")};overflow:hidden;display:flex"><div style="width:${pct}%;height:4px;border-radius:var(--radius-bar);background:${C(lv)};opacity:${stale ? 0.4 : 1}"></div></div>`;
  return `<div style="display:flex;align-items:center;gap:10px;padding-left:${indent}px;${F}">
<div style="width:52px;flex-shrink:0;font-size:10px;font-weight:500;letter-spacing:var(--tracking-caps);color:${C("lichen")};text-transform:uppercase">${label}</div>
${track}
<div style="width:38px;flex-shrink:0;text-align:right;font-size:var(--text-body);font-weight:500;color:${pct == null ? C("lichen") : pct < 15 ? C("ember") : C("granite")}">${pct == null ? "—" : pct + "%"}</div>
<div style="width:64px;flex-shrink:0;text-align:right;font-size:var(--text-caption);color:${C(metaColor)}">${meta}</div>
</div>`;
};

const groupHead = ({
  name,
  count,
  open = false,
  note = "",
  noteColor = "lichen",
}) =>
  `<div style="display:flex;align-items:center;gap:8px;${F}">${chevron(open)}
<div style="font-size:var(--text-body);font-weight:600;color:${C("granite")}">${name}</div>
<div style="font-size:var(--text-caption);color:${C("lichen")}">${count}</div>
<div style="flex:1"></div>
<div style="font-size:var(--text-caption);font-weight:500;color:${C(noteColor)}">${note}</div></div>`;

const group = (head, rows, extra = "") =>
  `<div style="display:flex;flex-direction:column;gap:8px;padding:12px 16px;border-top:1px solid ${C("frost")}">${head}${rows.join("")}${extra}</div>`;

const menuItem = (label, key = "") =>
  `<div style="display:flex;align-items:center;height:26px;padding:0 10px;border-radius:6px;${F}font-size:var(--text-body);color:${C("granite")}"><div style="flex:1">${label}</div><div style="color:${C("lichen")}">${key}</div></div>`;
