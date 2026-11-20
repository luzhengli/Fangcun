#!/usr/bin/env node
// 从 apps/desktop/src-tauri/icons/source/ 的 SVG 出全部应用图标（D-30）：
// 各尺寸 png、icon.icns（iconutil）、icon.ico（内嵌 png）。16 px 一档用 app-icon-16.svg（线宽加到 1 px）。
//
// 用法（在仓库根目录）：node tools/icons/build.js
// 需要 Playwright（同 tools/prototype-preview/check.js）和 macOS 自带的 iconutil。

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

let chromium;
try {
  ({ chromium } = require("playwright"));
} catch {
  console.error("找不到 Playwright：npm i -g playwright && npx playwright install chromium");
  process.exit(2);
}

const ROOT = path.resolve(__dirname, "..", "..");
const ICONS = path.join(ROOT, "apps", "desktop", "src-tauri", "icons");
const SRC = path.join(ICONS, "source");
const svgFor = (px) =>
  fs.readFileSync(path.join(SRC, px <= 16 ? "app-icon-16.svg" : "app-icon.svg"), "utf8");

(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage();
  const render = async (px) => {
    await page.setViewportSize({ width: px, height: px });
    const svg = svgFor(px).replace(/width="1024" height="1024"/, `width="${px}" height="${px}"`);
    await page.setContent(
      `<html><body style="margin:0;background:transparent">${svg}</body></html>`,
    );
    return page.screenshot({ omitBackground: true, clip: { x: 0, y: 0, width: px, height: px } });
  };
  const cache = new Map();
  const png = async (px) => {
    if (!cache.has(px)) cache.set(px, await render(px));
    return cache.get(px);
  };

  // Tauri 配置里列的几张。
  const out = { "32x32.png": 32, "128x128.png": 128, "128x128@2x.png": 256, "icon.png": 512 };
  for (const [name, px] of Object.entries(out))
    fs.writeFileSync(path.join(ICONS, name), await png(px));

  // icon.icns：先拼 iconset，再交给 iconutil。
  const set = `${fs.mkdtempSync(path.join(os.tmpdir(), "fangcun-"))}/icon.iconset`;
  fs.mkdirSync(set);
  for (const base of [16, 32, 128, 256, 512]) {
    fs.writeFileSync(path.join(set, `icon_${base}x${base}.png`), await png(base));
    fs.writeFileSync(path.join(set, `icon_${base}x${base}@2x.png`), await png(base * 2));
  }
  execFileSync("iconutil", ["-c", "icns", set, "-o", path.join(ICONS, "icon.icns")]);
  fs.rmSync(path.dirname(set), { recursive: true, force: true });

  // icon.ico：每个尺寸直接内嵌 png（Windows Vista 起支持）。
  const sizes = [16, 24, 32, 48, 64, 256];
  const imgs = [];
  for (const px of sizes) imgs.push(await png(px));
  const head = Buffer.alloc(6 + 16 * sizes.length);
  head.writeUInt16LE(0, 0);
  head.writeUInt16LE(1, 2);
  head.writeUInt16LE(sizes.length, 4);
  let offset = head.length;
  sizes.forEach((px, i) => {
    const e = 6 + 16 * i;
    head.writeUInt8(px >= 256 ? 0 : px, e);
    head.writeUInt8(px >= 256 ? 0 : px, e + 1);
    head.writeUInt8(0, e + 2);
    head.writeUInt8(0, e + 3);
    head.writeUInt16LE(1, e + 4);
    head.writeUInt16LE(32, e + 6);
    head.writeUInt32LE(imgs[i].length, e + 8);
    head.writeUInt32LE(offset, e + 12);
    offset += imgs[i].length;
  });
  fs.writeFileSync(path.join(ICONS, "icon.ico"), Buffer.concat([head, ...imgs]));

  await browser.close();
  console.log(`已生成：${Object.keys(out).concat(["icon.icns", "icon.ico"]).join("、")}`);
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
