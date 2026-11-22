// self-harness 的通用积木：读配置、读 markdown 条目、问 git 和 gh、看端口。
// 零依赖，Node 20 以上。项目专用的逻辑不要写进这里，写在项目自己的脚本里再 import 这些函数。

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, realpathSync, statSync } from "node:fs";
import { join, relative } from "node:path";

export const CONFIG_PATH = ".harness/config.json";

export function sh(cmd, args, cwd) {
  const r = spawnSync(cmd, args, { cwd, encoding: "utf8" });
  return { ok: r.status === 0, out: (r.stdout ?? "").trim(), err: (r.stderr ?? "").trim() };
}

export const exists = (root, file) => Boolean(file) && existsSync(join(root, file));
export const readText = (root, file) => readFileSync(join(root, file), "utf8");

export function readJson(root, file, fallback) {
  if (!exists(root, file)) return fallback;
  return JSON.parse(readText(root, file));
}

/** 项目配置。没有 `.harness/config.json` 时用探测结果，标出 `detected: true`。 */
export function loadConfig(root) {
  const cfg = readJson(root, CONFIG_PATH, null);
  if (cfg) return { ...cfg, detected: false };
  return { ...detect(root), detected: true };
}

const firstExisting = (root, list) => list.find((f) => exists(root, f));

/** 不写任何文件，只猜：入口文件、状态、待办、决定、摩擦在哪，检查命令是什么。 */
export function detect(root) {
  const docs = exists(root, "docs") ? "docs" : null;
  const inDocs = (names) => names.flatMap((n) => [n, docs ? `${docs}/${n}` : null]).filter(Boolean);
  const entry = uniqueFiles(
    root,
    ["AGENTS.md", "CLAUDE.md"].filter((f) => exists(root, f)),
  );
  // 根目录下的 markdown 也算文档：不少项目把计划、日志、编排说明放在根目录。
  const rootMd = readdirSync(root).filter(
    (f) => f.endsWith(".md") && statSync(join(root, f)).isFile(),
  );
  const pkg = readJson(root, "package.json", null);
  const scripts = pkg?.scripts ?? {};
  const pm = exists(root, "pnpm-lock.yaml") ? "pnpm" : exists(root, "yarn.lock") ? "yarn" : "npm";
  const check = ["check", "verify", "test"].find((s) => s in scripts);
  const decisionsInDocs = docs
    ? readdirSync(join(root, docs)).find((f) => /decision|决策|决定/i.test(f))
    : undefined;
  return {
    entry,
    status: firstExisting(root, inDocs(["STATUS.md", "PLAN.md", "progress.md", "PROGRESS.md"])),
    backlog: firstExisting(
      root,
      inDocs(["BACKLOG.md", "tasks.json", "feature_list.json", "TODO.md"]),
    ),
    decisions:
      firstExisting(root, inDocs(["DECISIONS.md"])) ??
      (decisionsInDocs ? `${docs}/${decisionsInDocs}` : undefined),
    friction: null,
    queues: [],
    docs: uniqueFiles(root, [docs, ...rootMd].filter(Boolean)),
    startup: [],
    check: check ? `${pm} run ${check}` : null,
    ports: [],
    boundaries: [],
    budgets: { entryLines: 120, statusLines: 40 },
  };
}

/**
 * markdown 里某个标题下没勾的 `- [ ]` 条目，读到同级或更高级的标题为止。
 * `heading` 写标题文字，不带 `#`。
 */
export function openItems(root, file, heading, { mark = " " } = {}) {
  if (!exists(root, file)) return [];
  const lines = readText(root, file).split("\n");
  const level = (l) => /^(#+) /.exec(l)?.[1].length ?? 0;
  const start = lines.findIndex((l) => level(l) > 0 && l.replace(/^#+ /, "").trim() === heading);
  if (start < 0) return [];
  const depth = level(lines[start]);
  const items = [];
  const re = new RegExp(`^- \\[${mark === " " ? " " : mark}\\] (.*)$`);
  for (const line of lines.slice(start + 1)) {
    const d = level(line);
    if (d > 0 && d <= depth) break;
    const m = re.exec(line);
    if (m) items.push(m[1]);
  }
  return items;
}

export function gitInfo(root) {
  const branch = sh("git", ["rev-parse", "--abbrev-ref", "HEAD"], root);
  if (!branch.ok) return null;
  const dirty = sh("git", ["status", "--porcelain"], root).out.split("\n").filter(Boolean).length;
  return { branch: branch.out, dirty };
}

/** 远端上还没合进 main（或 `base`）的分支。 */
export function unmergedBranches(root, base = "origin/main") {
  const r = sh(
    "git",
    [
      "branch",
      "-r",
      "--no-merged",
      base,
      "--format=%(refname:short)\t%(committerdate:short)\t%(subject)",
    ],
    root,
  );
  return r.out
    .split("\n")
    .filter((l) => l && !l.startsWith("origin/HEAD") && !l.startsWith("origin "))
    .map((l) => l.split("\t"));
}

/** 开着的 PR。gh 没装或没登录时返回 null。 */
export function openPrs(root, base = "main") {
  const r = sh(
    "gh",
    ["pr", "list", "--state", "open", "--json", "number,title,baseRefName,headRefName,isDraft"],
    root,
  );
  if (!r.ok) return null;
  return JSON.parse(r.out).map((p) => ({ ...p, offBase: p.baseRefName !== base }));
}

/** 占着某个端口的进程，带运行时长。忘了关的开发服务器会让新的启动失败。 */
export function portHolders(port) {
  const pids = sh("lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"]).out.split("\n");
  return pids.filter(Boolean).map((pid) => {
    const [etime, ...cmd] = sh("ps", ["-o", "etime=,command=", "-p", pid]).out.split(/\s+/);
    return { pid, etime, command: cmd.join(" ") };
  });
}

/** 列出目录下的 markdown 文件（递归，跳过 node_modules、.git、archive）。 */
export function markdownFiles(root, dirOrFile) {
  if (!exists(root, dirOrFile)) return [];
  const abs = join(root, dirOrFile);
  if (statSync(abs).isFile()) return dirOrFile.endsWith(".md") ? [dirOrFile] : [];
  const out = [];
  const walk = (dir) => {
    for (const name of readdirSync(dir)) {
      if (["node_modules", ".git", "archive"].includes(name)) continue;
      const p = join(dir, name);
      if (statSync(p).isDirectory()) walk(p);
      else if (name.endsWith(".md")) out.push(relative(root, p));
    }
  };
  walk(abs);
  return out;
}

/** 去掉指向同一个文件的路径（例如 CLAUDE.md 软链到 AGENTS.md）。 */
export function uniqueFiles(root, files) {
  const seen = new Set();
  return files.filter((f) => {
    if (!exists(root, f)) return true;
    const real = realpathSync(join(root, f));
    if (seen.has(real)) return false;
    seen.add(real);
    return true;
  });
}

export const short = (s, n = 70) => (s.length > n ? `${s.slice(0, n)}…` : s);

export function section(title, items, empty = "没有") {
  console.log(`\n${title}${items.length ? `（${items.length}）` : ""}`);
  if (items.length === 0) console.log(`  ${empty}`);
  for (const i of items) console.log(`  · ${i}`);
}
