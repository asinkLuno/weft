// 独立校验器：检查 weft story.yaml 的实体/事件引用完整性与归属无环，
// 并用简化公历近似复算锚点偏移时间（只做展平与进位，供人工核对 tick 顺序）。
// 用法：node check_story.mjs <story.yaml>（先用 npx js-yaml 转成 JSON 管道进来）

import { readFileSync } from 'node:fs';

const doc = JSON.parse(readFileSync(0, 'utf8'));
const errors = [];
const entities = doc.entity ?? {};
const links = doc.entity_link ?? {};
const events = doc.event ?? {};
const narratives = doc.narrative ?? {};

const has = (id, where) => {
  if (!(id in entities)) errors.push(`${where}: 引用不存在的实体 id「${id}」`);
};

// 1. belong_to 引用 + 无环
const parentOf = {};
for (const [id, e] of Object.entries(entities)) {
  for (const p of e.belong_to ?? []) {
    has(p, `belong_to(${id})`);
    (parentOf[id] ??= []).push(p);
  }
}
const state = new Map(); // 1=visiting 2=done
for (const root of Object.keys(entities)) {
  const stack = [[root, false]];
  while (stack.length) {
    const [id, leaving] = stack.pop();
    if (leaving) { state.set(id, 2); continue; }
    if (state.get(id) === 2) continue;
    if (state.get(id) === 1) { errors.push(`belong_to 成环：经过「${id}」`); continue; }
    state.set(id, 1);
    stack.push([id, true]);
    for (const p of parentOf[id] ?? []) stack.push([p, false]);
  }
}

// 2. entity_link：恰好两个实体且存在
for (const [group, pairs] of Object.entries(links)) {
    for (const [pair, i] of pairs.map((p, i) => [p, i])) {
    if (pair.entities?.length !== 2) errors.push(`entity_link(${group}[${i}]): entities 必须恰好两个，实际 ${pair.entities?.length}`);
    else for (const id of pair.entities) has(id, `entity_link(${group}[${i}])`);
  }
}

// 3. 事件实体引用
for (const [id, e] of Object.entries(events)) {
  for (const eid of e.entities ?? []) has(eid, `event(${id})`);
}

// 4. 叙事线引用
const seen = new Set();
for (const [name, ids] of Object.entries(narratives)) {
  for (const eid of ids) {
    if (!(eid in events)) errors.push(`narrative(${name}): 引用不存在的事件「${eid}」`);
    seen.add(eid);
  }
}
const orphans = Object.keys(events).filter((id) => !seen.has(id));

// 5. 时间：逐层校验分量数（每层 ≤ 6）→ 展平求和 → 公历近似归一化（月 12、日 31、时 24、分/秒 60，
//    仅用于核对相对顺序与进位，非历法真值）
const N = 6;
const flat = (v) => {
  if (typeof v === 'number') return [v];
  if (!Array.isArray(v) || v.length === 0) {
    errors.push(`时间既不是整数也不是非空数组：${JSON.stringify(v)}`);
    return null;
  }
  if (Array.isArray(v[0])) {
    const base = flat(v[0]);
    if (!base) return null;
    if (v.length - 1 > N) errors.push(`偏移层分量数 ${v.length - 1} 超过 ${N}：${JSON.stringify(v)}`);
    const sum = [...base, ...Array(N).fill(0)].slice(0, N);
    for (let i = 1; i < v.length && i <= N; i++) sum[i - 1] += v[i];
    return sum;
  }
  if (v.length > N) errors.push(`分量数 ${v.length} 超过 ${N}：${JSON.stringify(v)}`);
  if (v.some((x) => !Number.isInteger(x))) errors.push(`分量含非整数：${JSON.stringify(v)}`);
  return [...v, ...Array(N).fill(0)].slice(0, N).map(Number);
};
const norm = (c) => {
  const caps = [Infinity, 12, 31, 24, 60, 60];
  const v = [...c];
  for (let i = 5; i >= 1; i--) {
    while (v[i] < 1) { v[i] += caps[i]; v[i - 1] -= 1; }
    if (v[i] > caps[i]) { v[i - 1] += Math.floor((v[i] - 1) / caps[i]); v[i] = ((v[i] - 1) % caps[i]) + 1; }
  }
  return v;
};
const tick = (c) => c[0] * 372 + c[1] * 31 + c[2] + c[3] / 24 + c[4] / 1440 + c[5] / 86400;
const times = {};
for (const [id, e] of Object.entries(events)) {
  const s = flat(e.start_time);
  if (!s) continue;
  const sn = norm(s);
  times[id] = tick(sn);
  if (e.end_time !== undefined) {
    const en = flat(e.end_time);
    if (en && tick(norm(en)) < times[id]) errors.push(`event(${id}): end_time 早于 start_time`);
  }
}

// 报告
if (errors.length) {
  console.error(`✗ ${errors.length} 个问题：`);
  for (const e of errors) console.error('  - ' + e);
  process.exit(1);
}
console.log(`✓ 实体 ${Object.keys(entities).length} 个（根 ${Object.values(entities).filter((e) => !e.belong_to?.length).length} 个），关系组 ${Object.keys(links).length} 组，事件 ${Object.keys(events).length} 个，叙事线 ${Object.keys(narratives).length} 条`);
console.log(`  未进任何叙事线的事件：${orphans.length ? orphans.join('、') : '无'}`);
const sorted = Object.entries(times).sort((a, b) => a[1] - b[1]);
console.log(`  时间跨度：${sorted[0][0]} → ${sorted.at(-1)[0]}`);
for (const [id, t] of sorted) console.log(`    ${t.toFixed(2).padStart(10)}  ${id}`);
