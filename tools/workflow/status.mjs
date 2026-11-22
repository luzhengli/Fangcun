#!/usr/bin/env node

// 工作流现在卡在哪。通用部分（分支、PR、待人处理、摩擦、端口、find）交给 self-harness，
// 这里只放本项目自己的：原型版本、示例数据跟没跟上原型。说明见 docs/WORKFLOW.md「工具」。
//
// 用法（在仓库根目录）：
//   pnpm workflow                 打印状态（self-harness status，加上原型版本与示例数据）
//   pnpm workflow --fetch         先 git fetch，再看开着的 PR、云端推来还没合并的分支
//   pnpm workflow accept Main …   示例数据已经跟上这几张画板，记下画板现在的哈希
//   pnpm workflow accept Settings --fixture settings.ts
//                                 第一次给某张画板配示例数据：登记画板和对应的示例数据文件
//   pnpm workflow --ci            只报示例数据落后，按 GitHub Actions 的 ::warning:: 格式，不失败
//   pnpm workflow drift           只列示例数据落后的画板（self-harness status 调它）
//   pnpm workflow find 快照 保留   查这件事定没定过（self-harness find，范围在 .harness/config.json）。
//                                 问人之前、写代码之前跑（W-12）
//
// 只读，accept 只改 apps/desktop/src/ipc/fixtures/prototype-sync.json。

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const REPO = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const SYNC = join(REPO, "apps/desktop/src/ipc/fixtures/prototype-sync.json");
const HARNESS = join(REPO, "self-harness/scripts/harness.mjs");

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

const harness = (...args) =>
  process.exit(spawnSync("node", [HARNESS, ...args], { cwd: REPO, stdio: "inherit" }).status ?? 1);

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

if (argv[0] === "find") harness(...argv);

if (argv[0] === "drift") {
  for (const b of fixtureDrift())
    console.log(`${b.board} → fixtures/${b.fixture}，上次对齐 ${b.version}`);
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

const version = /\| 版本 \| (\S+) \|/.exec(read("prototype/README.md"))?.[1] ?? "?";
console.log(`原型版本 ${version}。`);
harness("status", ...argv);
