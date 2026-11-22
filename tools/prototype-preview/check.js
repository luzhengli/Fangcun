#!/usr/bin/env node
// 原型同步后的检查：用预览器渲染 prototype/project/canvas.json 里的每张画板，
// 有报错、残留 {{、不认识的标签时失败（退出码 1），说明预览器要跟着补。
// 同时把每张画板的默认状态截图写到 prototype/screens/<名称>.png，供本地实现时对照。
//
// 用法（在仓库根目录）：node tools/prototype-preview/check.js
// 只检查不截图：加 --no-screens
// 截图写到别的目录：加 --out <目录>（相对仓库根目录）。pnpm screens --compare 用它在本机重渲原型，
// 免得拿云端渲染的截图和本机的应用比，字体不同算出一片差异。
// 只渲染几张画板：加 --only Main,Global。只重写这几张的截图，别的截图不动（本机字体和云端不同，
// 全部重渲会让没改的画板截图也变）。
// 渲染别处的画板（例如画布上的草稿，不碰 prototype/）：加 --src <目录>，目录结构同 prototype/project/，
// 要有 canvas.json。一般和 --out 一起用。
// 需要 Playwright。claude.ai 云端会话里已装好；本地没有时运行 npm i -g playwright && npx playwright install chromium。
'use strict';
const fs = require('fs');
const path = require('path');

let chromium;
try { ({ chromium } = require('playwright')); } catch (e) {
  console.error('找不到 Playwright。云端会话里已预装；本地请先安装：npm i -g playwright && npx playwright install chromium');
  process.exit(2);
}

const ROOT = path.resolve(__dirname, '..', '..') + path.sep;
const PROJECT = path.join(ROOT, 'prototype', 'project');
const arg = (flag) => { const i = process.argv.indexOf(flag); return i >= 0 ? process.argv[i + 1] : undefined; };
const SRC = arg('--src') ? path.resolve(ROOT, arg('--src')) : PROJECT;
const ONLY = arg('--only') ? arg('--only').split(',').map((s) => s.trim().replace(/\.dc\.html$/, '')) : null;
const outAt = process.argv.indexOf('--out');
const SHOTS = outAt >= 0 ? path.resolve(ROOT, process.argv[outAt + 1]) : path.join(ROOT, 'prototype', 'screens');
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.css': 'text/css', '.svg': 'image/svg+xml', '.png': 'image/png' };
const noScreens = process.argv.includes('--no-screens');

(async () => {
  const canvas = JSON.parse(fs.readFileSync(path.join(SRC, 'canvas.json'), 'utf8'));
  const listed = canvas.order.filter((f) => f.endsWith('.dc.html') && !f.includes('/ds/'));
  const boards = ONLY ? listed.filter((f) => ONLY.includes(f.replace(/\.dc\.html$/, ''))) : listed;
  if (ONLY && boards.length < ONLY.length) {
    console.error('canvas.json 里没有：' + ONLY.filter((n) => !boards.includes(n + '.dc.html')).join('、'));
    process.exit(2);
  }
  // 被别的画板 <dc-import> 的子组件也是画板，单独渲染一样要能过。
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  // 不开服务器：把 http://preview.local/ 映射到仓库目录。
  await page.route('http://preview.local/**', (route) => {
    const u = new URL(route.request().url());
    let f = path.normalize(path.join(ROOT, decodeURIComponent(u.pathname)));
    if (!f.startsWith(ROOT)) return route.fulfill({ status: 403, body: '' });
    // --src：预览器读 prototype/project/ 的请求改去读那个目录。
    // 那个目录里没有的文件（被引用的子画板、设计系统）仍读 prototype/project/。
    if (SRC !== PROJECT && f.startsWith(PROJECT + path.sep)) {
      const alt = path.join(SRC, path.relative(PROJECT, f));
      if (fs.existsSync(alt)) f = alt;
    }
    if (f.endsWith(path.sep)) f = path.join(f, 'index.html');
    if (!fs.existsSync(f)) return route.fulfill({ status: 404, body: '' });
    route.fulfill({ status: 200, contentType: (TYPES[path.extname(f)] || 'application/octet-stream') + '; charset=utf-8', body: fs.readFileSync(f) });
  });
  // 字体走网络，检查时不等它。
  await page.route('https://fonts.googleapis.com/**', (r) => r.fulfill({ status: 200, contentType: 'text/css', body: '' }));

  if (!noScreens) fs.mkdirSync(SHOTS, { recursive: true });
  let failed = 0;
  for (const file of boards) {
    const name = file.replace(/\.dc\.html$/, '');
    const b = canvas.boards[file] || {};
    const errs = [];
    const onErr = (e) => errs.push('脚本出错：' + e.message);
    page.on('pageerror', onErr);
    await page.setViewportSize({ width: b.w || 1440, height: Math.min(b.h || 900, 900) });
    await page.goto('http://preview.local/tools/prototype-preview/?board=' + encodeURIComponent(name));
    await page.waitForFunction(() => window.__dcPreview && window.__dcPreview.ready, null, { timeout: 15000 }).catch(() => errs.push('15 秒内没有渲染完'));
    await page.waitForTimeout(200);
    const issues = await page.evaluate(() => (window.__dcPreview ? window.__dcPreview.issues : ['预览器没有加载']));
    const all = errs.concat(issues);
    page.off('pageerror', onErr);
    if (all.length) { failed++; console.log('✗ ' + file + '\n    ' + all.join('\n    ')); }
    else console.log('✓ ' + file);
    if (!noScreens) {
      await page.evaluate(() => { const bar = document.getElementById('pv-issues'); if (bar) bar.remove(); });
      await page.screenshot({ path: path.join(SHOTS, name + '.png'), fullPage: true });
    }
  }
  // 画布上已经删掉的画板，截图也删掉。只渲染了几张时不删。
  if (!noScreens && !ONLY) {
    const keep = new Set(boards.map((f) => f.replace(/\.dc\.html$/, '') + '.png'));
    for (const f of fs.readdirSync(SHOTS)) if (f.endsWith('.png') && !keep.has(f)) fs.unlinkSync(path.join(SHOTS, f));
  }
  await browser.close();
  console.log(failed ? '\n' + failed + ' 张画板没通过：先补 tools/prototype-preview/dc-preview.js，再提交。' : '\n全部通过，共 ' + boards.length + ' 张画板。');
  process.exit(failed ? 1 : 0);
})().catch((e) => { console.error(e); process.exit(1); });
