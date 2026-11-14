#!/usr/bin/env node
// 给真实的桌面窗口截图，包括系统画的部分（红黄绿按钮、标题栏），不靠操控桌面。
//
// pnpm screens 截的是浏览器里的界面，看不到窗口外壳。这个脚本找到正在运行的方寸窗口，
// 把它提到前台（按钮才是彩色的），用 macOS 自带的 screencapture 按窗口编号截图，再：
//   - 写出整窗截图 screens/window.png；
//   - 把左上角放大 4 倍写到 screens/window-topleft.png，看窗口按钮和工具栏对不对齐；
//   - 找到红色关闭按钮，报出它的中心在第几 pt，和工具栏按钮的中心比。
//
// 用法（在仓库根目录）：
//   pnpm window-shot              窗口已经开着（pnpm dev 或装好的应用）
//   pnpm window-shot --launch     自己起 pnpm dev，等窗口出来、内容加载完，截完关掉
//   --settle <秒>                 窗口出现后等多久再截，默认 10（第一次全盘扫描约 7 秒）
//
// 第一次运行要给终端（或 VS Code）「屏幕录制」权限：系统设置 › 隐私与安全性 › 屏幕与系统录音。
// 没有权限时脚本会说，并且不截图。只读，不改任何文件，截图目录不进 git。

import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, openSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { cropZoom, findColor } from "./lib/image.mjs";

const APP = join(dirname(fileURLToPath(import.meta.url)), "..");
const REPO = join(APP, "..", "..");
const OUT = join(APP, "screens");

// 开发包的进程名是 Cargo 包名，打包后的是 productName。
const OWNERS = ["fangcun-desktop", "Fangcun", "方寸"];
// 对齐的目标：工具栏里后退、侧栏按钮的中心，实测 19.25pt（几何中线是 19.5pt：40px 高、底边 1px 线）。
// 取实测值让一排按钮在同一条线上，见 DECISIONS 2026-11-14「窗口按钮的位置」。
const TOOLBAR_MID_PT = 19.25;
// macOS 关闭按钮的红色。
const CLOSE_RED = [255, 95, 87];

const argv = process.argv.slice(2);
const launch = argv.includes("--launch");
const settleAt = argv.indexOf("--settle");
const settle = settleAt >= 0 ? Number(argv[settleAt + 1]) : 10;

if (process.platform !== "darwin") {
  console.error("只能在 macOS 上运行。");
  process.exit(2);
}

// 用 JXA 调 CoreGraphics 列窗口。不需要任何权限；但没有屏幕录制权限时拿不到窗口标题。
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// onScreen 为 false 时也列出别的桌面空间里、被遮住的窗口。
function listWindows(onScreen = true) {
  const option = onScreen
    ? "$.kCGWindowListOptionOnScreenOnly | $.kCGWindowListExcludeDesktopElements"
    : "$.kCGWindowListOptionAll";
  const script = `
    ObjC.import('CoreGraphics');
    const list = ObjC.castRefToObject($.CGWindowListCopyWindowInfo(${option}, $.kCGNullWindowID));
    const out = [];
    for (let i = 0; i < list.count; i++) {
      const w = list.objectAtIndex(i);
      const get = (k) => { const v = w.objectForKey(k); return v.isNil() ? null : v.js; };
      const b = w.objectForKey('kCGWindowBounds');
      out.push({ id: get('kCGWindowNumber'), owner: get('kCGWindowOwnerName'), pid: get('kCGWindowOwnerPID'),
        layer: get('kCGWindowLayer'), name: get('kCGWindowName'),
        x: b.objectForKey('X').js, y: b.objectForKey('Y').js, w: b.objectForKey('Width').js, h: b.objectForKey('Height').js });
    }
    JSON.stringify(out);`;
  const r = spawnSync("osascript", ["-l", "JavaScript", "-e", script], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`列窗口失败：${r.stderr}`);
  return JSON.parse(r.stdout);
}

// 先按进程名找到方寸的进程，提到前台（别的桌面空间里的窗口会被带过来，按钮也变成彩色），
// 再在屏幕上的窗口里取最大的那个。
async function findAppWindow() {
  const pids = new Set(
    listWindows(false)
      .filter((w) => OWNERS.includes(w.owner))
      .map((w) => w.pid),
  );
  for (const pid of pids) activate(pid);
  if (pids.size > 0) await sleep(800);
  const windows = listWindows();
  const mine = windows
    .filter((w) => w.layer === 0 && OWNERS.includes(w.owner) && w.w > 300 && w.h > 300)
    .sort((a, b) => b.w * b.h - a.w * a.h);
  // 有屏幕录制权限时，别的应用的窗口标题读得到；一个都读不到，就是没权限。
  const permitted = windows.some((w) => w.layer === 0 && w.name);
  return { window: mine[0] ?? null, permitted };
}

function activate(pid) {
  const script = `ObjC.import('AppKit');
    $.NSRunningApplication.runningApplicationWithProcessIdentifier(${pid}).activateWithOptions($.NSApplicationActivateAllWindows);`;
  spawnSync("osascript", ["-l", "JavaScript", "-e", script]);
}

let dev = null;
if (launch && spawnSync("lsof", ["-iTCP:1420", "-sTCP:LISTEN", "-t"]).stdout.length > 0) {
  console.error("已经有 pnpm dev 在跑（1420 端口被占）。打开它的窗口后，不加 --launch 再运行。");
  process.exit(1);
}
if (launch) {
  mkdirSync(join(REPO, "target"), { recursive: true });
  const logFile = join(REPO, "target", "window-shot-dev.log");
  const log = openSync(logFile, "w");
  console.log(`起 pnpm dev，输出在 ${relative(REPO, logFile)}。第一次要编译，可能要几分钟。`);
  dev = spawn("pnpm", ["dev"], { cwd: REPO, detached: true, stdio: ["ignore", log, log] });
}
const stopDev = () => {
  if (dev) {
    try {
      process.kill(-dev.pid, "SIGTERM");
    } catch {}
  }
};
process.on("SIGINT", () => {
  stopDev();
  process.exit(130);
});

let exitCode = 0;
try {
  let found = await findAppWindow();
  if (launch) {
    const deadline = Date.now() + 15 * 60 * 1000;
    while (!found.window && Date.now() < deadline) {
      if (dev.exitCode !== null)
        throw new Error("pnpm dev 退出了，看日志 target/window-shot-dev.log");
      await sleep(2000);
      found = await findAppWindow();
    }
  }
  if (!found.window) {
    throw new Error(
      launch
        ? "15 分钟内没等到窗口。"
        : "没找到方寸的窗口。先 pnpm dev，或加 --launch 让脚本自己起。",
    );
  }
  if (!found.permitted) {
    throw new Error(
      "终端没有「屏幕录制」权限，截出来只有空窗口。到 系统设置 › 隐私与安全性 › 屏幕与系统录音，" +
        "给运行这个脚本的终端（或 VS Code）打开，重启它再试。",
    );
  }

  if (launch) {
    console.log(`窗口出来了，等 ${settle} 秒让内容加载完。`);
    await sleep(settle * 1000);
  }
  const win = found.window;
  activate(win.pid);
  await sleep(800);
  // 等内容加载时窗口可能被挪动或换了大小，按编号重取一次位置。
  Object.assign(win, listWindows().find((w) => w.id === win.id) ?? {});

  mkdirSync(OUT, { recursive: true });
  const shot = join(OUT, "window.png");
  const cap = spawnSync("screencapture", ["-x", "-o", `-l${win.id}`, shot], { encoding: "utf8" });
  if (cap.status !== 0) throw new Error(`screencapture 失败：${cap.stderr}`);

  const dims = spawnSync("sips", ["-g", "pixelWidth", "-g", "pixelHeight", shot], {
    encoding: "utf8",
  }).stdout;
  const pixelWidth = Number(/pixelWidth: (\d+)/.exec(dims)?.[1]);
  const scale = pixelWidth / win.w;

  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    const topLeft = join(OUT, "window-topleft.png");
    await cropZoom(page, {
      src: shot,
      out: topLeft,
      x: 0,
      y: 0,
      w: Math.round(240 * scale),
      h: Math.round(48 * scale),
      zoom: 4,
    });
    const red = await findColor(page, {
      src: shot,
      rgb: CLOSE_RED,
      region: { x: 0, y: 0, w: Math.round(60 * scale), h: Math.round(48 * scale) },
    });

    console.log(`窗口 ${win.owner}（编号 ${win.id}）${win.w}×${win.h} pt，截图 ${scale}x`);
    console.log(`整窗：${relative(REPO, shot)}`);
    console.log(`左上角放大 4 倍：${relative(REPO, topLeft)}`);
    if (red) {
      const cy = red.center.y / scale;
      const off = cy - TOOLBAR_MID_PT;
      console.log(
        `关闭按钮中心 y = ${cy.toFixed(2)} pt，工具栏按钮中心 ${TOOLBAR_MID_PT} pt，` +
          `${Math.abs(off) < 0.5 ? "对齐" : `偏${off < 0 ? "上" : "下"} ${Math.abs(off).toFixed(2)} pt`}。`,
      );
    } else {
      console.log("没找到红色的关闭按钮。窗口可能没在前台（按钮是灰的），看放大图判断。");
    }
  } finally {
    await browser.close();
  }
} catch (error) {
  console.error(error.message);
  exitCode = 1;
} finally {
  stopDev();
}
process.exit(exitCode);
