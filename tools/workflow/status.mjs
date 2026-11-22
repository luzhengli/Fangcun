#!/usr/bin/env node

// 工作流现在卡在哪：一条命令看完要接手的东西。说明见 docs/WORKFLOW.md「pnpm workflow」。
//
// 用法（在仓库根目录）：
//   pnpm workflow                 打印状态
//   pnpm workflow --fetch         先 git fetch，再看开着的 PR、云端推来还没合并的分支
//   pnpm workflow accept Main …   示例数据已经跟上这几张画板，记下画板现在的哈希
//   pnpm workflow accept Settings --fixture settings.ts
//                                 第一次给某张画板配示例数据：登记画板和对应的示例数据文件
//   pnpm workflow --ci            只报示例数据落后，按 GitHub Actions 的 ::warning:: 格式，不失败
//   pnpm workflow find 快照 保留   查这件事定没定过：设计文档第 7 节与 9.2、9.3，DECISIONS、BACKLOG
//                                 里同时含这几个词的行，带章节。问人之前、写代码之前跑（W-12）。
//                                 先按文件报处数；超过 40 处时，没列出的也报出章节，加 --all 全列
//
// 只读，accept 只改 apps/desktop/src/ipc/fixtures/prototype-sync.json。

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const SYNC = join(REPO, "apps/desktop/src/ipc/fixtures/prototype-sync.json");

const read = (file) => readFileSync(join(REPO, file), "utf8");
const hashBoard = (board) =>
  createHash("sha256")
    .update(readFileSync(join(REPO, `prototype/project/${board}.dc.html`)))
    .digest("hex")
    .slice(0, 12);

// 示例数据落后于原型的画板：画板文件的哈希和示例数据上次对齐时记下的不一样。
function fixtureDrift() {
  const sync = JSON.parse(readFileSync(SYNC, "utf8"));
  return Object.entries(sync.boards)
    .map(([board, entry]) => ({ board, ...entry, now: hashBoard(board) }))
    .filter((b) => b.now !== b.hash);
}

// 某个标题（任意级别，如 `## 待人处理`、`### 摩擦记录`）下没勾的 `- [ ]` 条目，读到同级或更高级的标题为止。
function openItems(file, heading) {
  if (!existsSync(join(REPO, file))) return [];
  const lines = read(file).split("\n");
  const level = (l) => /^(#+) /.exec(l)?.[1].length ?? 0;
  const start = lines.findIndex((l) => level(l) > 0 && l.replace(/^#+ /, "").trim() === heading);
  if (start < 0) return [];
  const depth = level(lines[start]);
  const items = [];
  for (const line of lines.slice(start + 1)) {
    const d = level(line);
    if (d > 0 && d <= depth) break;
    const m = /^- \[ \] (.*)$/.exec(line);
    if (m) items.push(m[1]);
  }
  return items;
}

const git = (...args) => spawnSync("git", args, { cwd: REPO, encoding: "utf8" });

function unmergedBranches() {
  const r = git(
    "branch",
    "-r",
    "--no-merged",
    "origin/main",
    "--format=%(refname:short)\t%(committerdate:short)\t%(subject)",
  );
  return r.stdout
    .split("\n")
    .filter((l) => l && !l.startsWith("origin/HEAD"))
    .map((l) => l.split("\t"));
}

// 占着 pnpm dev 端口 1420 的进程：忘了关的旧 `pnpm dev` 会让新的启动失败，还可能连着真实的 ~/.fangcun。
function devPort() {
  const pids = spawnSync("lsof", ["-nP", "-iTCP:1420", "-sTCP:LISTEN", "-t"], { encoding: "utf8" })
    .stdout.split("\n")
    .filter(Boolean);
  return pids.map((pid) => {
    const ps = spawnSync("ps", ["-o", "etime=,command=", "-p", pid], {
      encoding: "utf8",
    }).stdout.trim();
    const [etime, ...cmd] = ps.split(/\s+/);
    return `PID ${pid}，已跑 ${etime}  ${cmd.join(" ").slice(0, 60)}`;
  });
}

// 开着的 PR。base 不是 main 的单独标出来：交接规则第 6 条要求都以 main 为基（W-15）。
function openPrs() {
  const r = spawnSync(
    "gh",
    ["pr", "list", "--state", "open", "--json", "number,title,baseRefName,headRefName,isDraft"],
    { cwd: REPO, encoding: "utf8" },
  );
  if (r.status !== 0) return null;
  return JSON.parse(r.stdout).map(
    (p) =>
      `#${p.number} ${p.headRefName}${p.baseRefName === "main" ? "" : ` → ${p.baseRefName}（base 应为 main）`}${p.isDraft ? "（草稿）" : ""}  ${p.title}`,
  );
}

const short = (s, n = 60) => (s.length > n ? `${s.slice(0, n)}…` : s);

const argv = process.argv.slice(2);

if (argv[0] === "accept") {
  const fx = argv.indexOf("--fixture");
  const fixture = fx > 0 ? argv[fx + 1] : undefined;
  const boards = argv.slice(1).filter((_, i) => i + 1 !== fx && i + 1 !== fx + 1);
  const sync = JSON.parse(readFileSync(SYNC, "utf8"));
  const missing = boards.filter((b) => !existsSync(join(REPO, `prototype/project/${b}.dc.html`)));
  const unknown = boards.filter((b) => !(b in sync.boards));
  if (boards.length === 0 || missing.length > 0 || (unknown.length > 0 && !fixture)) {
    console.error(
      missing.length > 0
        ? `prototype/project/ 里没有这张画板：${missing.join("、")}`
        : unknown.length > 0
          ? `${unknown.join("、")} 还没登记示例数据。第一次要写对应的文件：pnpm workflow accept ${unknown[0]} --fixture <apps/desktop/src/ipc/fixtures/ 下的文件名>`
          : `要写画板名，已登记：${Object.keys(sync.boards).join("、")}`,
    );
    process.exit(2);
  }
  if (fixture && !existsSync(join(REPO, "apps/desktop/src/ipc/fixtures", fixture))) {
    console.error(`apps/desktop/src/ipc/fixtures/ 下没有 ${fixture}`);
    process.exit(2);
  }
  for (const b of unknown) sync.boards[b] = { fixture };
  const version = /\| 版本 \| (\S+) \|/.exec(read("prototype/README.md"))?.[1] ?? "?";
  for (const b of boards) Object.assign(sync.boards[b], { hash: hashBoard(b), version });
  writeFileSync(SYNC, `${JSON.stringify(sync, null, 2)}\n`);
  console.log(`已记下：${boards.join("、")} 对齐到原型版本 ${version}。`);
  process.exit(0);
}

// 查「定没定过」：每行都要含全部关键词（不分大小写）。设计文档只看技术选型和已拍板的章节。
if (argv[0] === "find") {
  const words = argv.slice(1).map((w) => w.toLowerCase());
  if (words.length === 0) {
    console.error("要写关键词，例如：pnpm workflow find 索引 SQLite");
    process.exit(2);
  }
  const scopes = [
    { file: "docs/product-design.md", keep: (h) => /^#{2,3} (7\.|9\.2|9\.3)/.test(h) },
    { file: "docs/DECISIONS.md", keep: () => true },
    { file: "docs/BACKLOG.md", keep: () => true },
  ];
  const all = argv.includes("--all");
  const hits = [];
  for (const { file, keep } of scopes) {
    let heading = "";
    read(file)
      .split("\n")
      .forEach((line, i) => {
        if (/^#{2,3} /.test(line)) heading = line;
        if (!keep(heading) || !words.every((w) => line.toLowerCase().includes(w))) return;
        hits.push({ file, line: i + 1, heading: heading.replace(/^#+ /, ""), text: line.trim() });
      });
  }
  if (hits.length === 0) {
    console.log("没找到。换个说法再查一次；确实没有就是没定过，按 AGENTS.md 先问人。");
    process.exit(0);
  }
  // 先报每个文件几处，结果多时也不会整份漏看（W-16）。
  const byFile = Object.groupBy(hits, (h) => h.file);
  console.log(
    `共 ${hits.length} 处：${Object.entries(byFile)
      .map(([f, hs]) => `${f} ${hs.length}`)
      .join("，")}\n`,
  );
  const LIMIT = 40;
  const shown = all ? hits : hits.slice(0, LIMIT);
  for (const h of shown)
    console.log(`${h.file}:${h.line}  ${h.heading}\n    ${short(h.text, 160)}`);
  if (shown.length < hits.length) {
    const rest = Object.groupBy(hits.slice(LIMIT), (h) => `${h.file}  ${h.heading}`);
    console.log(
      `\n另有 ${hits.length - LIMIT} 处没列出，在这些章节（加 --all 全列，或加关键词缩小）：`,
    );
    for (const [where, hs] of Object.entries(rest))
      console.log(`  ${where}：第 ${hs.map((h) => h.line).join("、")} 行`);
  }
  process.exit(0);
}

if (argv.includes("--ci")) {
  for (const b of fixtureDrift()) {
    console.log(
      `::warning file=apps/desktop/src/ipc/fixtures/${b.fixture}::示例数据落后于原型 ${b.board}（上次对齐 ${b.version}）。跟上后运行 pnpm workflow accept ${b.board}`,
    );
  }
  process.exit(0);
}

if (argv.includes("--fetch")) git("fetch", "--quiet", "--prune", "origin");

const version = /\| 版本 \| (\S+) \|/.exec(read("prototype/README.md"))?.[1] ?? "?";
const drift = fixtureDrift();
const human = openItems("docs/BACKLOG.md", "待人处理");
const proto = openItems("docs/BACKLOG.md", "原型待改");
const friction = openItems("docs/WORKFLOW.md", "摩擦记录");
const branches = unmergedBranches();
const head = git("rev-parse", "--abbrev-ref", "HEAD").stdout.trim();
const dirty = git("status", "--porcelain").stdout.trim().split("\n").filter(Boolean).length;

const section = (title, items, empty) => {
  console.log(`\n${title}${items.length ? `（${items.length}）` : ""}`);
  if (items.length === 0) console.log(`  ${empty}`);
  for (const i of items) console.log(`  · ${i}`);
};

console.log(`分支 ${head}，未提交的改动 ${dirty} 处。原型版本 ${version}。`);
section(
  "待人处理",
  human.map((i) => short(i)),
  "没有",
);
if (argv.includes("--fetch")) {
  const prs = openPrs();
  section("开着的 PR", prs ?? [], prs ? "没有" : "查不到（gh 没装或没登录）");
}
section(
  "云端推来、还没合并的分支",
  branches.map(([b, d, s]) => `${b}  ${d}  ${short(s, 50)}`),
  "没有（加 --fetch 先拉一次）",
);
section(
  "原型待改（在终端会话里用 Artifact 工具改画布）",
  proto.map((i) => short(i)),
  "没有",
);
section(
  "示例数据落后于原型",
  drift.map((b) => `${b.board} → fixtures/${b.fixture}，上次对齐 ${b.version}`),
  "没有",
);
section(
  "没复盘的流程摩擦",
  friction.map((i) => short(i)),
  "没有",
);
const port = devPort();
if (port.length > 0) section("端口 1420 被占着（pnpm dev 会启动失败，确认是谁的再关）", port, "");
if (friction.length >= 5)
  console.log("  已经攒了 5 条以上，该复盘工作流了（docs/WORKFLOW.md「复盘」）。");
