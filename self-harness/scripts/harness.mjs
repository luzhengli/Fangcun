#!/usr/bin/env node
// self-harness 的命令行。零依赖，Node 20 以上。说明见同目录上一级的 SKILL.md。
//
//   全部命令都可加 --config <文件>：用别处的配置试跑，不在目标项目里落盘。
//
//   node harness.mjs status   [--root 目录] [--fetch]   现在卡在哪：分支、PR、待人处理、摩擦、端口
//   node harness.mjs audit    [--root 目录]             harness 体检：常驻长度、文档、规则档位、判官缺口、断链
//   node harness.mjs metrics  [--root 目录] [--days 30] [--record] [--json]
//   node harness.mjs find     <关键词…> [--all]        查一件事定没定过
//   node harness.mjs check    [--root 目录]             机判：清单格式、边界词。不过时退出码 1，给 CI 和 lint 用
//   node harness.mjs init     [--root 目录] [--write]   探测项目，打印建议的配置；--write 才落盘，且不覆盖
//
// 只读，除了 init --write 和 metrics --record（只写 .harness/ 下）。

import { spawnSync } from "node:child_process";
import { appendFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  CONFIG_PATH,
  detect,
  exists,
  gitInfo,
  loadConfig,
  markdownFiles,
  openItems,
  openPrs,
  portHolders,
  readText,
  section,
  sh,
  short,
  uniqueFiles,
  unmergedBranches,
} from "./lib/core.mjs";
import {
  countBy,
  GRADES,
  LEVELS,
  loadOracles,
  loadRules,
  ORACLES_PATH,
  RULES_PATH,
  ruleSuggestions,
  validate,
} from "./lib/inventory.mjs";

const SKILL = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const argv = process.argv.slice(2);
const cmd = argv[0];
const opt = (flag) => {
  const i = argv.indexOf(flag);
  return i >= 0 ? argv[i + 1] : undefined;
};
const flag = (f) => argv.includes(f);
const root = resolve(opt("--root") ?? process.cwd());
// --config：用别处的配置文件试跑，不在目标项目里落盘（例如在别的项目上先只读试一次）。
const cfg = opt("--config")
  ? { ...JSON.parse(readFileSync(resolve(opt("--config")), "utf8")), detected: false }
  : loadConfig(root);
const lines = (file) => readText(root, file).split("\n").length;
const chars = (file) => [...readText(root, file)].length;

function note() {
  if (cfg.detected)
    console.log(`（没有 ${CONFIG_PATH}，用的是探测结果。运行 init 看建议的配置。）`);
}

// ── status ──────────────────────────────────────────────────────────────
function status() {
  if (flag("--fetch")) sh("git", ["fetch", "--quiet", "--prune", "origin"], root);
  const g = gitInfo(root);
  console.log(g ? `分支 ${g.branch}，未提交的改动 ${g.dirty} 处。` : "不是 git 仓库。");
  note();
  for (const q of cfg.queues ?? [])
    section(
      q.title,
      openItems(root, q.file, q.heading).map((i) => short(i)),
    );
  if (flag("--fetch")) {
    const prs = openPrs(root);
    section(
      "开着的 PR",
      (prs ?? []).map(
        (p) =>
          `#${p.number} ${p.headRefName}${p.offBase ? ` → ${p.baseRefName}（base 应为 main）` : ""}${p.isDraft ? "（草稿）" : ""}  ${short(p.title, 50)}`,
      ),
      prs ? "没有" : "查不到（gh 没装或没登录）",
    );
    section(
      "远端还没合并的分支",
      unmergedBranches(root).map(([b, d, s]) => `${b}  ${d}  ${short(s, 50)}`),
    );
  }
  for (const x of cfg.extra ?? []) {
    const r = spawnSync(x.run, { cwd: root, shell: true, encoding: "utf8" });
    const out = (r.stdout ?? "").split("\n").filter(Boolean);
    section(x.title, out);
  }
  for (const port of cfg.ports ?? []) {
    const held = portHolders(port);
    if (held.length)
      section(
        `端口 ${port} 被占着（确认是谁的再关）`,
        held.map((h) => `PID ${h.pid}，已跑 ${h.etime}  ${short(h.command, 60)}`),
      );
  }
  if (cfg.friction) {
    const f = openItems(root, cfg.friction.file, cfg.friction.heading);
    section(
      "没复盘的摩擦",
      f.map((i) => short(i)),
    );
    if (f.length >= (cfg.friction.threshold ?? 5))
      console.log("  已经攒够了，该复盘了（references/evolve.md）。");
  }
}

// ── audit ───────────────────────────────────────────────────────────────
function audit() {
  note();
  const budgets = { entryLines: 120, statusLines: 40, ...(cfg.budgets ?? {}) };
  const findings = [];

  // 常驻：每次会话都装进上下文的文件。
  const entry = uniqueFiles(
    root,
    (cfg.entry ?? []).filter((f) => exists(root, f)),
  );
  console.log("\n常驻上下文（平台自动加载）");
  let entryLines = 0;
  for (const f of entry) {
    const n = lines(f);
    entryLines += n;
    console.log(`  ${f}  ${n} 行  ${chars(f)} 字符`);
    const sections = readText(root, f)
      .split(/\n(?=## )/)
      .filter((s) => s.startsWith("## "))
      .map((s) => ({ title: s.split("\n")[0].slice(3), n: s.split("\n").length }))
      .sort((a, b) => b.n - a.n)
      .slice(0, 4);
    if (sections.length)
      console.log(`    最长的几节：${sections.map((s) => `${s.title} ${s.n} 行`).join("，")}`);
  }
  // 开工必读：入口文件要求每次开工先读的文件。和自动加载的一起，才是每次会话的固定开销。
  const startup = uniqueFiles(
    root,
    (cfg.startup ?? []).filter((f) => exists(root, f)),
  ).filter((f) => !entry.includes(f));
  const fixed = [...entry, ...startup].reduce((a, f) => a + chars(f), 0);
  if (startup.length) {
    console.log("\n开工必读（入口文件要求每次先读）");
    for (const f of startup) console.log(`  ${f}  ${lines(f)} 行  ${chars(f)} 字符`);
  } else if (!cfg.startup?.length) {
    findings.push("没配置 startup：入口文件里要求每次开工先读的文件填进去，固定开销才算得准");
  }
  console.log(`\n每次会话的固定开销：${fixed} 字符（自动加载 + 开工必读）`);
  if (entryLines > budgets.entryLines)
    findings.push(
      `常驻 ${entryLines} 行，超过预算 ${budgets.entryLines}。只在特定场合用到的流程挪成 skill（references/context.md）`,
    );

  // 状态文件。
  if (cfg.status && exists(root, cfg.status)) {
    const n = lines(cfg.status);
    const last = sh("git", ["log", "-1", "--format=%H", "--", cfg.status], root).out;
    const since = last ? sh("git", ["rev-list", "--count", `${last}..HEAD`], root).out : "?";
    console.log(`\n状态文件 ${cfg.status}：${n} 行，上次改动后又有 ${since} 个提交`);
    if (n > budgets.statusLines)
      findings.push(`${cfg.status} ${n} 行，超过预算 ${budgets.statusLines}`);
    if (Number(since) > 10)
      findings.push(`${cfg.status} 落后 ${since} 个提交，可能过期。能算出来的状态交给 status 命令`);
  }

  // 文档体量。
  const docs = [...new Set((cfg.docs ?? []).flatMap((d) => markdownFiles(root, d)))];
  const sized = docs.map((f) => ({ f, n: lines(f), c: chars(f) })).sort((a, b) => b.c - a.c);
  console.log("\n文档体量（前 8）");
  for (const d of sized.slice(0, 8)) console.log(`  ${d.f}  ${d.n} 行  ${d.c} 字符`);

  // 待办里已完成条目的占比。
  if (cfg.backlog?.endsWith(".md") && exists(root, cfg.backlog)) {
    const ls = readText(root, cfg.backlog).split("\n");
    const sum = (p) => ls.filter(p).reduce((a, l) => a + [...l].length, 0);
    const done = sum((l) => /^- \[x\]/i.test(l));
    const all = sum(() => true);
    const pct = Math.round((done / all) * 100);
    console.log(`\n${cfg.backlog}：已完成条目占 ${pct}%`);
    if (pct > 50)
      findings.push(
        `${cfg.backlog} 有 ${pct}% 是已完成条目，按里程碑归档（references/context.md）`,
      );
  }

  // 断链：文档里的相对链接指向不存在的文件。
  const broken = [];
  for (const f of docs) {
    const text = readText(root, f);
    for (const m of text.matchAll(/\]\(([^)\s]+)\)/g)) {
      const target = m[1].split("#")[0];
      if (!target || /^[a-z]+:/i.test(target)) continue;
      const p = join(dirname(f), target);
      if (!exists(root, p)) broken.push(`${f} → ${m[1]}`);
    }
  }
  section("断掉的相对链接", broken);
  if (broken.length) findings.push(`${broken.length} 处断链`);

  // 规则清单。
  const rules = loadRules(root);
  if (!rules) {
    findings.push(`没有 ${RULES_PATH}：规则还没盘点成数据（references/rules.md）`);
  } else {
    const by = countBy(rules, (r) => r.level);
    console.log(
      `\n规则 ${rules.length} 条，按档：${Object.keys(LEVELS)
        .map((l) => `${l} ${LEVELS[l]} ${by[l] ?? 0}`)
        .join("，")}`,
    );
    const { upgrade, prune } = ruleSuggestions(rules);
    section(
      "升档候选（文字规则，被违反过）",
      upgrade.map(
        (r) =>
          `${r.id} 第 ${r.level} 档，违反 ${r.violations.length} 次：${short(r.text, 50)}${r.next ? ` → ${short(r.next, 40)}` : ""}`,
      ),
    );
    section(
      "删除或降档候选（常驻、查询档，从没被违反，没写 keep、next）",
      prune.map((r) => `${r.id} 第 ${r.level} 档：${short(r.text, 60)}`),
    );
    if (upgrade.length) findings.push(`${upgrade.length} 条规则该升档`);
  }

  // 判官清单。
  const oracles = loadOracles(root);
  if (!oracles) {
    findings.push(`没有 ${ORACLES_PATH}：质量维度还没列判官（references/oracles.md）`);
  } else {
    const by = countBy(oracles, (o) => o.grade);
    console.log(
      `\n判官 ${oracles.length} 个维度：${GRADES.map((g) => `${g} ${by[g] ?? 0}`).join("，")}`,
    );
    section(
      "没有判官的维度",
      oracles
        .filter((o) => o.grade === "无")
        .map((o) => `[${o.group ?? "-"}] ${o.dimension}${o.next ? ` → ${short(o.next, 40)}` : ""}`),
    );
    const none = by["无"] ?? 0;
    if (none) findings.push(`${none} 个质量维度没有判官`);
  }

  // 摩擦。
  if (cfg.friction) {
    const n = openItems(root, cfg.friction.file, cfg.friction.heading).length;
    if (n >= (cfg.friction.threshold ?? 5)) findings.push(`${n} 条摩擦没复盘`);
  } else findings.push("没配置摩擦记录在哪（references/evolve.md）");

  const b = boundaryViolations();
  section("边界词", b);
  if (b.length) findings.push(`${b.length} 处越界`);

  section("结论", findings, "没有要处理的");
}

// ── 边界：某个目录里不许出现的词（例如孵化中的通用代码不许写项目专有名词）。
function boundaryViolations() {
  const out = [];
  for (const b of cfg.boundaries ?? []) {
    for (const word of b.forbidden) {
      const r = sh("grep", ["-rnF", "--exclude-dir=node_modules", word, b.dir], root);
      for (const l of r.out.split("\n").filter(Boolean)) out.push(`「${word}」 ${short(l, 90)}`);
    }
  }
  return out;
}

// ── check ───────────────────────────────────────────────────────────────
// skill 自己的版本：SKILL.md description 里的「版本 x.y.z」要和 CHANGELOG 最新一节一致。
function versionProblems() {
  const desc = /版本 (\d+\.\d+\.\d+)/.exec(readText(SKILL, "SKILL.md"))?.[1];
  const log = /^## (\d+\.\d+\.\d+)/m.exec(readText(SKILL, "CHANGELOG.md"))?.[1];
  if (!desc) return ["SKILL.md 的 description 里没写「版本 x.y.z」"];
  if (desc !== log) return [`SKILL.md 写的是版本 ${desc}，CHANGELOG 最新是 ${log}`];
  return [];
}

function check() {
  const problems = [
    ...validate(loadRules(root), loadOracles(root)),
    ...boundaryViolations().map((v) => `越界 ${v}`),
    ...versionProblems(),
  ];
  if (problems.length) {
    console.error(`self-harness check：${problems.length} 处问题`);
    for (const p of problems) console.error(`  · ${p}`);
    process.exit(1);
  }
  console.log("self-harness check 通过");
}

// ── metrics ─────────────────────────────────────────────────────────────
function metrics() {
  const days = Number(opt("--days") ?? 30);
  const since = `${days}.days.ago`;
  const docPrefixes = (cfg.docs ?? []).map((d) => d.replace(/\/$/, ""));
  const exclude = cfg.metricsExclude ?? ["pnpm-lock.yaml", "package-lock.json", "yarn.lock"];
  const isDoc = (p) => docPrefixes.some((d) => p === d || p.startsWith(`${d}/`));
  const log = sh(
    "git",
    ["log", `--since=${since}`, "--no-merges", "--numstat", "--format=@%s"],
    root,
  ).out;
  let commits = 0;
  let reverts = 0;
  const churn = { docs: 0, code: 0 };
  for (const l of log.split("\n")) {
    if (l.startsWith("@")) {
      commits += 1;
      if (/^@revert/i.test(l)) reverts += 1;
      continue;
    }
    const [a, d, p] = l.split("\t");
    if (!p || a === "-" || exclude.some((e) => p.startsWith(e) || p.endsWith(e))) continue;
    churn[isDoc(p) ? "docs" : "code"] += Number(a) + Number(d);
  }
  const docShare = churn.docs + churn.code ? churn.docs / (churn.docs + churn.code) : 0;

  // Agent 自己定的取舍有多少被人推翻：按配置里的两个模式数行，粗估。
  let decisions = null;
  if (cfg.decisionsMetric) {
    const files = [cfg.decisions, cfg.backlog].filter((f) => f && exists(root, f));
    const text = files.map((f) => readText(root, f)).join("\n");
    const count = (re) => text.split("\n").filter((l) => new RegExp(re).test(l)).length;
    decisions = {
      agent: count(cfg.decisionsMetric.agent),
      overturned: count(cfg.decisionsMetric.overturned),
    };
  }

  const friction = cfg.friction
    ? readText(root, cfg.friction.file)
        .split("\n")
        .filter((l) => /^- \[[ x]\] (\d{4}-\d{2}-\d{2})/.test(l))
        .map((l) => l.slice(6, 16))
    : [];
  const cutoff = new Date(Date.now() - days * 864e5).toISOString().slice(0, 10);
  const recentFriction = friction.filter((d) => d >= cutoff).length;

  const fixedFiles = uniqueFiles(root, [...(cfg.entry ?? []), ...(cfg.startup ?? [])]).filter((f) =>
    exists(root, f),
  );
  const entryChars = fixedFiles.reduce((a, f) => a + chars(f), 0);
  const rules = loadRules(root);
  const oracles = loadOracles(root);
  const m = {
    date: new Date().toISOString().slice(0, 10),
    days,
    commits,
    reverts,
    docShare: Math.round(docShare * 100),
    frictionPerWeek: Math.round((recentFriction / days) * 7 * 10) / 10,
    entryChars,
    decisions,
    rulesByLevel: rules ? countBy(rules, (r) => r.level) : null,
    oraclesByGrade: oracles ? countBy(oracles, (o) => o.grade) : null,
  };
  if (flag("--record")) {
    mkdirSync(join(root, ".harness"), { recursive: true });
    appendFileSync(join(root, ".harness/metrics.jsonl"), `${JSON.stringify(m)}\n`);
  }
  if (flag("--json")) return console.log(JSON.stringify(m, null, 2));
  note();
  console.log(`近 ${days} 天，${commits} 个提交（不含合并）`);
  console.log(`  改文档的行数占全部改动 ${m.docShare}%（harness 维护成本）`);
  console.log(`  revert ${reverts} 次`);
  console.log(`  摩擦 ${recentFriction} 条，每周 ${m.frictionPerWeek} 条`);
  console.log(`  每次会话的固定开销 ${entryChars} 字符（自动加载 + 开工必读）`);
  if (decisions)
    console.log(
      `  Agent 定的取舍 ${decisions.agent} 处，其中被推翻 ${decisions.overturned} 处（按关键词粗估）`,
    );
  if (m.rulesByLevel)
    console.log(
      `  规则按档：${Object.entries(m.rulesByLevel)
        .map(([l, n]) => `${l}:${n}`)
        .join(" ")}`,
    );
  if (m.oraclesByGrade)
    console.log(
      `  判官：${Object.entries(m.oraclesByGrade)
        .map(([g, n]) => `${g} ${n}`)
        .join("，")}`,
    );
  console.log(
    "\n只看趋势，不设目标（references/metrics.md）。加 --record 记一笔到 .harness/metrics.jsonl。",
  );
}

// ── find ────────────────────────────────────────────────────────────────
function find() {
  const words = argv
    .slice(1)
    .filter((w) => !w.startsWith("--"))
    .map((w) => w.toLowerCase());
  if (!words.length) {
    console.error("要写关键词：harness.mjs find 快照 保留");
    process.exit(2);
  }
  const scopes =
    cfg.find ?? [cfg.decisions, cfg.backlog, cfg.status].filter(Boolean).map((file) => ({ file }));
  const hits = [];
  for (const { file, headings } of scopes) {
    if (!exists(root, file)) continue;
    let heading = "";
    readText(root, file)
      .split("\n")
      .forEach((line, i) => {
        if (/^#{1,3} /.test(line)) heading = line;
        if (headings && !new RegExp(headings).test(heading)) return;
        if (!words.every((w) => line.toLowerCase().includes(w))) return;
        hits.push({ file, line: i + 1, heading: heading.replace(/^#+ /, ""), text: line.trim() });
      });
  }
  if (!hits.length) return console.log("没找到。换个说法再查；确实没有就是没定过，先问人。");
  const byFile = countBy(hits, (h) => h.file);
  console.log(
    `共 ${hits.length} 处：${Object.entries(byFile)
      .map(([f, n]) => `${f} ${n}`)
      .join("，")}\n`,
  );
  const LIMIT = 40;
  const shown = flag("--all") ? hits : hits.slice(0, LIMIT);
  for (const h of shown)
    console.log(`${h.file}:${h.line}  ${h.heading}\n    ${short(h.text, 160)}`);
  if (shown.length < hits.length) {
    console.log(`\n另有 ${hits.length - LIMIT} 处没列出（--all 全列）：`);
    const rest = {};
    for (const h of hits.slice(LIMIT)) {
      const key = `${h.file}  ${h.heading}`;
      rest[key] = [...(rest[key] ?? []), h.line];
    }
    for (const [w, ls] of Object.entries(rest)) console.log(`  ${w}：第 ${ls.join("、")} 行`);
  }
}

// ── init ────────────────────────────────────────────────────────────────
function init() {
  const d = detect(root);
  console.log("探测到：");
  console.log(`  入口文件  ${d.entry.join("、") || "没有（建议建 AGENTS.md）"}`);
  console.log(`  状态      ${d.status ?? "没有"}`);
  console.log(`  待办      ${d.backlog ?? "没有"}`);
  console.log(`  决定      ${d.decisions ?? "没有"}`);
  console.log(`  检查命令  ${d.check ?? "没有（这是第一个要补的判官）"}`);
  console.log(
    "  要人补    startup（入口文件要求每次先读的文件）、friction（摩擦记在哪）、queues（要人处理的队列）",
  );
  console.log("\n建议的 .harness/config.json（摩擦记录、队列要人补）：\n");
  console.log(JSON.stringify(d, null, 2));
  if (!flag("--write")) return console.log("\n没写文件。确认后加 --write。");
  mkdirSync(join(root, ".harness"), { recursive: true });
  const put = (file, body) => {
    if (exists(root, file)) return console.log(`已有 ${file}，不覆盖`);
    writeFileSync(join(root, file), body);
    console.log(`写了 ${file}`);
  };
  put(CONFIG_PATH, `${JSON.stringify(d, null, 2)}\n`);
  put(RULES_PATH, readText(SKILL, "templates/rules.json"));
  put(ORACLES_PATH, readText(SKILL, "templates/oracles.json"));
}

const commands = { status, audit, metrics, find, check, init };
if (!commands[cmd]) {
  console.error(`用法：harness.mjs <${Object.keys(commands).join("|")}> [--root 目录]`);
  process.exit(2);
}
commands[cmd]();
