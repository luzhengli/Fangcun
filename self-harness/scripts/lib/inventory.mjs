// 规则清单与判官清单：读、校验、汇总。格式见 references/rules.md、references/oracles.md。

import { readJson } from "./core.mjs";

export const RULES_PATH = ".harness/rules.json";
export const ORACLES_PATH = ".harness/oracles.json";

export const LEVELS = {
  1: "写不出来",
  2: "做错就拦",
  3: "适时提醒",
  4: "按需加载",
  5: "常驻规则",
  6: "查询文档",
};
export const GRADES = ["机判", "机证", "人判", "无"];

export function loadRules(root) {
  return readJson(root, RULES_PATH, null)?.rules ?? null;
}

export function loadOracles(root) {
  return readJson(root, ORACLES_PATH, null)?.oracles ?? null;
}

/** 校验两份清单的字段。返回问题列表，空数组表示通过。 */
export function validate(rules, oracles) {
  const problems = [];
  const ids = new Set();
  for (const r of rules ?? []) {
    if (!r.id || !r.text) problems.push(`规则缺 id 或 text：${JSON.stringify(r).slice(0, 60)}`);
    if (ids.has(r.id)) problems.push(`规则 id 重复：${r.id}`);
    ids.add(r.id);
    if (!(r.level in LEVELS)) problems.push(`${r.id} 的 level 要是 1 到 6：${r.level}`);
    for (const v of r.violations ?? [])
      if (!/^\d{4}-\d{2}-\d{2}/.test(v.date ?? ""))
        problems.push(`${r.id} 的违反记录缺日期：${JSON.stringify(v)}`);
  }
  for (const o of oracles ?? []) {
    if (!o.dimension) problems.push(`判官缺 dimension：${JSON.stringify(o).slice(0, 60)}`);
    if (!GRADES.includes(o.grade))
      problems.push(`「${o.dimension}」的 grade 要是 ${GRADES.join("/")}`);
  }
  return problems;
}

/**
 * 升档候选：停在第 4 档及以下（文字规则）却被违反过的。按违反次数排。
 * 删除候选：第 5、6 档、从没被违反过、没写 `keep` 理由、也没写升档方案 `next` 的。
 * 没人守的规则「没被违反」可能只是「没被发现」，所以写了 `next` 的不算删除候选。
 */
export function ruleSuggestions(rules) {
  const upgrade = rules
    .filter((r) => r.level >= 4 && (r.violations ?? []).length > 0)
    .sort((a, b) => b.violations.length - a.violations.length);
  const prune = rules.filter(
    (r) => r.level >= 5 && !(r.violations ?? []).length && !r.keep && !r.next,
  );
  return { upgrade, prune };
}

export function countBy(list, key) {
  const out = {};
  for (const x of list) out[key(x)] = (out[key(x)] ?? 0) + 1;
  return out;
}
