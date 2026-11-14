#!/usr/bin/env node

// 工作流现在卡在哪：一条命令看完要接手的东西。说明见 docs/WORKFLOW.md「pnpm workflow」。
//
// 用法（在仓库根目录）：
//   pnpm workflow                 打印状态
//   pnpm workflow --fetch         先 git fetch，再看云端推来、还没合并的分支
//   pnpm workflow accept Main …   示例数据已经跟上这几张画板，记下画板现在的哈希
//   pnpm workflow --ci            只报示例数据落后，按 GitHub Actions 的 ::warning:: 格式，不失败
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

// 某个 `## 标题` 下没勾的 `- [ ]` 条目。
function openItems(file, heading) {
  if (!existsSync(join(REPO, file))) return [];
  const lines = read(file).split("\n");
  const start = lines.findIndex((l) => l.trim() === `## ${heading}`);
  if (start < 0) return [];
  const items = [];
  for (const line of lines.slice(start + 1)) {
    if (line.startsWith("## ")) break;
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

const short = (s, n = 60) => (s.length > n ? `${s.slice(0, n)}…` : s);

const argv = process.argv.slice(2);

if (argv[0] === "accept") {
  const boards = argv.slice(1);
  const sync = JSON.parse(readFileSync(SYNC, "utf8"));
  const unknown = boards.filter((b) => !(b in sync.boards));
  if (boards.length === 0 || unknown.length > 0) {
    console.error(`要写画板名，可选：${Object.keys(sync.boards).join("、")}`);
    process.exit(2);
  }
  const version = /\| 版本 \| (\S+) \|/.exec(read("prototype/README.md"))?.[1] ?? "?";
  for (const b of boards) Object.assign(sync.boards[b], { hash: hashBoard(b), version });
  writeFileSync(SYNC, `${JSON.stringify(sync, null, 2)}\n`);
  console.log(`已记下：${boards.join("、")} 对齐到原型版本 ${version}。`);
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
section(
  "云端推来、还没合并的分支",
  branches.map(([b, d, s]) => `${b}  ${d}  ${short(s, 50)}`),
  "没有（加 --fetch 先拉一次）",
);
section(
  "原型待改（到云端会话里改画布）",
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
if (friction.length >= 5)
  console.log("  已经攒了 5 条以上，该复盘工作流了（docs/WORKFLOW.md「复盘」）。");
