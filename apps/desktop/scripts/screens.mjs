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
//                      pnpm screens Main@codex …  截非默认状态的场景（W-14），场景表在 scripts/scenes.mjs。
//                          原型和应用走同样的几步点击。--compare 时另出一张放大的局部对比
//                          compare-<场景>-focus.png（默认是检查器，弹窗场景是弹窗）。
//                          不带参数时截全部画板和全部场景；走不通的场景报出来，接着截别的。
// 需要 Playwright 的浏览器：pnpm exec playwright install chromium --only-shell

import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, extname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";
import { compareImages } from "./lib/image.mjs";
import { DEFAULT_FOCUS, runStep, SCENES } from "./scenes.mjs";

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
  Settings: "/settings",
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
const unknown = wanted.filter((name) => !(name in BOARDS) && !(name in SCENES));
if (unknown.length > 0) {
  console.error(
    `没有这些画板或场景：${unknown.join("、")}。\n画板：${Object.keys(BOARDS).join("、")}\n场景：${Object.keys(SCENES).join("、")}`,
  );
  process.exit(2);
}
const names = wanted.length > 0 ? wanted : [...Object.keys(BOARDS), ...Object.keys(SCENES)];
// 每一项：画板、路由、要走的步骤、局部放大的区域。
const entries = names.map((name) => {
  const scene = SCENES[name];
  const board = scene ? scene.board : name;
  return {
    name,
    board,
    route: BOARDS[board],
    steps: scene?.steps ?? [],
    focus: scene ? (scene.focus ?? DEFAULT_FOCUS) : null,
  };
});

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
const failed = [];

mkdirSync(OUT, { recursive: true });
try {
  for (const entry of entries) {
    const { name, board, route, steps, focus } = entry;
    const frame = canvas.boards[`${board}.dc.html`] ?? {};
    await page.setViewportSize({ width: frame.w ?? 1440, height: Math.min(frame.h ?? 900, 900) });
    await page.goto(`${origin}/#${route}`);
    await page.reload(); // 只改 # 后面的部分不会重新加载，每张都从干净的状态开始。
    await page.waitForLoadState("networkidle");
    await page.evaluate(() => document.fonts.ready);
    const problem = await runSteps(page, steps);
    if (problem) {
      failed.push(`${name}（应用）：${problem}`);
      continue;
    }

    const file = join(OUT, `${snapshotFile ? "real-" : ""}${name}${dark ? "-dark" : ""}.png`);
    await page.screenshot({ path: file, fullPage: true });
    const focusFile = focus ? await shootFocus(page, focus, file) : null;
    shots.push({ ...entry, file, focusFile });
    if (!compare) {
      const proto =
        steps.length > 0 ? "（场景，原型用 --compare 重渲）" : `prototype/screens/${name}.png`;
      console.log(`${relative(REPO, file)}  ←→  ${proto}`);
    }
  }
  if (compare) await compareAll(browser, shots);
} finally {
  await browser.close();
  await server.close();
}

if (failed.length > 0) {
  console.error(`\n走不通的场景（界面还没做，或者文案和原型不一样）：\n  ${failed.join("\n  ")}`);
}
if (errors.length > 0) {
  console.error(`\n页面脚本出错：\n  ${[...new Set(errors)].join("\n  ")}`);
  process.exit(1);
}

async function runSteps(page, steps) {
  for (const step of steps) {
    const problem = await runStep(page, step);
    if (problem) return problem;
  }
  return null;
}

// 局部区域单独截一张，找不到就不截。
async function shootFocus(page, selector, file) {
  const el = page.locator(selector).first();
  if ((await el.count()) === 0 || !(await el.isVisible())) return null;
  const out = file.replace(/\.png$/, "-focus.png");
  await el.screenshot({ path: out });
  return out;
}

// 本机重渲原型（走同样的步骤），再逐张和应用截图比。
async function compareAll(browser, shots) {
  const protoDir = join(OUT, "prototype");
  mkdirSync(protoDir, { recursive: true });
  const proto = await browser.newPage();
  // 不开服务器：把 http://preview.local/ 映射到仓库目录，和 check.js 一样。
  await proto.route("http://preview.local/**", (route) => {
    const url = new URL(route.request().url());
    let file = join(REPO, decodeURIComponent(url.pathname));
    if (!file.startsWith(REPO)) return route.fulfill({ status: 403, body: "" });
    if (file.endsWith("/")) file = join(file, "index.html");
    if (!existsSync(file)) return route.fulfill({ status: 404, body: "" });
    const type =
      {
        ".html": "text/html",
        ".js": "text/javascript",
        ".json": "application/json",
        ".css": "text/css",
        ".svg": "image/svg+xml",
        ".png": "image/png",
      }[extname(file)] ?? "application/octet-stream";
    return route.fulfill({
      status: 200,
      contentType: `${type}; charset=utf-8`,
      body: readFileSync(file),
    });
  });
  await proto.route("https://fonts.googleapis.com/**", (route) =>
    route.fulfill({ status: 200, contentType: "text/css", body: "" }),
  );
  const canvasPage = await browser.newPage();
  console.log("差异是不同像素的占比。已知的环境差异（窗口按钮、滚动条预留）总会占一点。\n");
  for (const { name, board, steps, file, focusFile, focus } of shots) {
    const frame = canvas.boards[`${board}.dc.html`] ?? {};
    await proto.setViewportSize({ width: frame.w ?? 1440, height: Math.min(frame.h ?? 900, 900) });
    await proto.goto(
      `http://preview.local/tools/prototype-preview/?board=${encodeURIComponent(board)}`,
    );
    await proto.waitForFunction(() => window.__dcPreview?.ready, null, { timeout: 15000 });
    await proto.evaluate(() => document.getElementById("pv-issues")?.remove());
    const problem = await runSteps(proto, steps);
    if (problem) {
      console.log(`${name.padEnd(22)} 原型这边走不通：${problem}`);
      continue;
    }
    const protoFile = join(protoDir, `${name}.png`);
    await proto.screenshot({ path: protoFile, fullPage: true });

    const out = join(OUT, `compare-${name}.png`);
    const { ratio, sizeA, sizeB } = await compareImages(canvasPage, {
      a: protoFile,
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
      `${name.padEnd(22)} ${(ratio * 100).toFixed(1).padStart(5)}%  ${relative(REPO, out)}${size}`,
    );

    // 局部放大：两边各截自己的那块区域，原尺寸对比。
    const protoFocus = focus ? await shootFocus(proto, focus, protoFile) : null;
    if (focusFile && protoFocus) {
      const fout = join(OUT, `compare-${name}-focus.png`);
      const r = await compareImages(canvasPage, {
        a: protoFocus,
        b: focusFile,
        labelA: "原型",
        labelB: "应用",
        out: fout,
        scale: 1,
      });
      console.log(
        `${"".padEnd(22)} ${(r.ratio * 100).toFixed(1).padStart(5)}%  ${relative(REPO, fout)}（局部）`,
      );
    }
  }
}
