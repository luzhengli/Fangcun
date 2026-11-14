#!/usr/bin/env node
// 给应用的每个页面截图，用来和原型截图对比。
//
// 截图条件与 tools/prototype-preview/check.js 一致：画板的宽度、最高 900 的视口、浅色、整页。
// 应用用 fixtures 模式起，数据是照抄原型的示例数据（src/ipc/fixtures/）。
// 截图写到 apps/desktop/screens/<画板名>.png，与 prototype/screens/<画板名>.png 同名，逐张对着看。
// 这个目录不进 git。
//
// 用法（在仓库根目录）：pnpm screens            全部页面
//                      pnpm screens Main Global  只截这几张
//                      pnpm screens --snapshot target/ipc-snapshot.json
//                          用本机真实数据截图，写到 screens/real-<画板>.png。快照由
//                          cargo run -p fangcun-core --example ipc_snapshot -- target/ipc-snapshot.json
//                      pnpm screens --dark …   按系统深色外观截图，文件名加 -dark 生成
//                      pnpm screens --compare [画板…]
//                          截完再在本机重渲原型（screens/prototype/），逐张算差异，写出三栏拼图
//                          screens/compare-<画板>.png（原型 · 应用 · 差异标红）。两边同一台机器、
//                          同一个浏览器渲染，字体一样，差异才有意义。不能和 --snapshot、--dark 一起用。
// 需要 Playwright 的浏览器：pnpm exec playwright install chromium --only-shell

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import { compareImages } from "./lib/image.mjs";

const APP = join(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = join(APP, "..", "..");
const OUT = join(APP, "screens");

// 画板 → 应用里的路由。设计系统画板没有对应的页面。
const BOARDS = {
  Main: "/projects",
  Checkup: "/checkup",
  Library: "/library",
  Bundles: "/bundles",
  Global: "/global",
  Activity: "/activity",
};

const argv = process.argv.slice(2);
const snapshotAt = argv.indexOf("--snapshot");
const snapshotFile = snapshotAt >= 0 ? argv[snapshotAt + 1] : null;
if (snapshotAt >= 0) argv.splice(snapshotAt, 2);
const darkAt = argv.indexOf("--dark");
const dark = darkAt >= 0;
if (dark) argv.splice(darkAt, 1);
const compareAt = argv.indexOf("--compare");
const compare = compareAt >= 0;
if (compare) argv.splice(compareAt, 1);
if (compare && (snapshotFile || dark)) {
  console.error("--compare 只比示例数据、浅色的截图，不能和 --snapshot、--dark 一起用。");
  process.exit(2);
}
const wanted = argv;
const unknown = wanted.filter((name) => !(name in BOARDS));
if (unknown.length > 0) {
  console.error(`没有这些画板：${unknown.join("、")}。可选：${Object.keys(BOARDS).join("、")}`);
  process.exit(2);
}
const names = wanted.length > 0 ? wanted : Object.keys(BOARDS);

const canvas = JSON.parse(readFileSync(join(REPO, "prototype/project/canvas.json"), "utf8"));

const server = await createServer({
  root: APP,
  mode: "fixtures",
  logLevel: "error",
  // 不占 Tauri 开发用的 1420，随便找一个空端口。
  server: { port: 0, strictPort: false, host: "127.0.0.1" },
});
await server.listen();
const address = server.httpServer?.address();
if (!address || typeof address === "string") throw new Error("开发服务器没有起来");
const origin = `http://127.0.0.1:${address.port}`;

const browser = await chromium.launch();
const page = await browser.newPage({ colorScheme: dark ? "dark" : "light" });
if (snapshotFile) {
  const snapshot = readFileSync(join(REPO, snapshotFile), "utf8");
  await page.addInitScript(`window.__FANGCUN_SNAPSHOT__ = ${snapshot};`);
}
const errors = [];
page.on("pageerror", (error) => errors.push(error.message));
const shots = [];

mkdirSync(OUT, { recursive: true });
try {
  for (const name of names) {
    const board = canvas.boards[`${name}.dc.html`] ?? {};
    await page.setViewportSize({ width: board.w ?? 1440, height: Math.min(board.h ?? 900, 900) });
    await page.goto(`${origin}/#${BOARDS[name]}`);
    await page.reload(); // 只改 # 后面的部分不会重新加载，每张都从干净的状态开始。
    await page.waitForLoadState("networkidle");
    await page.evaluate(() => document.fonts.ready);

    const file = join(OUT, `${snapshotFile ? "real-" : ""}${name}${dark ? "-dark" : ""}.png`);
    await page.screenshot({ path: file, fullPage: true });
    shots.push({ name, file });
    if (!compare) console.log(`${relative(REPO, file)}  ←→  prototype/screens/${name}.png`);
  }
  if (compare) await compareAll(browser, shots);
} finally {
  await browser.close();
  await server.close();
}

if (errors.length > 0) {
  console.error(`\n页面脚本出错：\n  ${[...new Set(errors)].join("\n  ")}`);
  process.exit(1);
}

// 本机重渲原型，再逐张和应用截图比。
async function compareAll(browser, shots) {
  const protoDir = join(OUT, "prototype");
  const rendered = spawnSync(
    process.execPath,
    [join(REPO, "tools/prototype-preview/check.js"), "--out", relative(REPO, protoDir)],
    { cwd: REPO, encoding: "utf8" },
  );
  if (rendered.status !== 0) {
    throw new Error(`本机重渲原型没通过：\n${rendered.stdout}${rendered.stderr}`);
  }
  const canvasPage = await browser.newPage();
  console.log("差异是不同像素的占比。已知的环境差异（窗口按钮、滚动条预留）总会占一点。\n");
  for (const { name, file } of shots) {
    const out = join(OUT, `compare-${name}.png`);
    const { ratio, sizeA, sizeB } = await compareImages(canvasPage, {
      a: join(protoDir, `${name}.png`),
      b: file,
      labelA: "原型",
      labelB: "应用",
      out,
    });
    const size =
      sizeA.join("×") === sizeB.join("×")
        ? ""
        : `  尺寸不同：原型 ${sizeA.join("×")}，应用 ${sizeB.join("×")}`;
    console.log(
      `${name.padEnd(9)} ${(ratio * 100).toFixed(1).padStart(5)}%  ${relative(REPO, out)}${size}`,
    );
  }
}
