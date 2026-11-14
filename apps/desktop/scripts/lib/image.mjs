// 截图的图像处理：对比、拼图、裁切放大、找颜色。
//
// 不引入图像库，借 Playwright 起的浏览器里的 canvas 来算。调用方传一个已经打开的 page，
// 本模块只往里面塞 data URL、取回结果。

import { readFileSync, writeFileSync } from "node:fs";

const dataUrl = (file) => `data:image/png;base64,${readFileSync(file).toString("base64")}`;
const writeDataUrl = (file, url) =>
  writeFileSync(file, Buffer.from(url.slice(url.indexOf(",") + 1), "base64"));

// 一个像素算「不同」的阈值：RGB 任一通道差超过它。抗锯齿的轻微差别不算。
const THRESHOLD = 32;

/**
 * 对比两张图，写出「左 · 中 · 右」三栏拼图：a、b、差异（不同的像素标红）。
 * 两张图高度不同时，短的那张下面按白色补齐，多出来的部分全算不同。
 * 返回不同像素的占比和两张图的尺寸。
 */
export async function compareImages(page, { a, b, labelA, labelB, out, scale = 0.5 }) {
  const result = await page.evaluate(
    async ({ a, b, labelA, labelB, scale, threshold }) => {
      const load = (src) =>
        new Promise((resolve, reject) => {
          const img = new Image();
          img.onload = () => resolve(img);
          img.onerror = () => reject(new Error("图片读不出来"));
          img.src = src;
        });
      const [ia, ib] = await Promise.all([load(a), load(b)]);
      const w = Math.max(ia.width, ib.width);
      const h = Math.max(ia.height, ib.height);
      const pixels = (img) => {
        const c = new OffscreenCanvas(w, h);
        const ctx = c.getContext("2d");
        ctx.fillStyle = "#fff";
        ctx.fillRect(0, 0, w, h);
        ctx.drawImage(img, 0, 0);
        return ctx.getImageData(0, 0, w, h).data;
      };
      const da = pixels(ia);
      const db = pixels(ib);
      const diff = new ImageData(w, h);
      let changed = 0;
      for (let i = 0; i < da.length; i += 4) {
        const d = Math.max(
          Math.abs(da[i] - db[i]),
          Math.abs(da[i + 1] - db[i + 1]),
          Math.abs(da[i + 2] - db[i + 2]),
        );
        if (d > threshold) {
          changed++;
          diff.data.set([220, 38, 38, 255], i);
        } else {
          // 相同的地方淡化成浅灰，只留轮廓，红色才显眼。
          const lum = da[i] * 0.3 + da[i + 1] * 0.59 + da[i + 2] * 0.11;
          const g = 255 - (255 - lum) * 0.25;
          diff.data.set([g, g, g, 255], i);
        }
      }
      const diffCanvas = new OffscreenCanvas(w, h);
      diffCanvas.getContext("2d").putImageData(diff, 0, 0);

      const pad = 16;
      const head = 32;
      const cw = Math.round(w * scale);
      const ch = Math.round(h * scale);
      const outCanvas = document.createElement("canvas");
      outCanvas.width = cw * 3 + pad * 4;
      outCanvas.height = ch + head + pad;
      const ctx = outCanvas.getContext("2d");
      ctx.fillStyle = "#f4f4f2";
      ctx.fillRect(0, 0, outCanvas.width, outCanvas.height);
      ctx.font = "14px -apple-system, 'PingFang SC', sans-serif";
      ctx.fillStyle = "#333";
      const ratio = changed / (w * h);
      const labels = [
        `${labelA}  ${ia.width}×${ia.height}`,
        `${labelB}  ${ib.width}×${ib.height}`,
        `差异  ${(ratio * 100).toFixed(1)}%`,
      ];
      [ia, ib, diffCanvas].forEach((img, i) => {
        const x = pad + i * (cw + pad);
        ctx.fillText(labels[i], x, 22);
        ctx.fillStyle = "#fff";
        ctx.fillRect(x, head, cw, ch);
        ctx.fillStyle = "#333";
        const iw = img === diffCanvas ? w : img.width;
        const ih = img === diffCanvas ? h : img.height;
        ctx.drawImage(img, x, head, Math.round(iw * scale), Math.round(ih * scale));
      });
      return {
        ratio,
        sizeA: [ia.width, ia.height],
        sizeB: [ib.width, ib.height],
        png: outCanvas.toDataURL("image/png"),
      };
    },
    { a: dataUrl(a), b: dataUrl(b), labelA, labelB, scale, threshold: THRESHOLD },
  );
  writeDataUrl(out, result.png);
  return { ratio: result.ratio, sizeA: result.sizeA, sizeB: result.sizeB };
}

/** 从图里裁一块（像素坐标）并放大，最近邻插值，看清对齐用。 */
export async function cropZoom(page, { src, out, x, y, w, h, zoom = 4 }) {
  const url = await page.evaluate(
    async ({ src, x, y, w, h, zoom }) => {
      const img = new Image();
      img.src = src;
      await img.decode();
      const c = document.createElement("canvas");
      c.width = w * zoom;
      c.height = h * zoom;
      const ctx = c.getContext("2d");
      ctx.imageSmoothingEnabled = false;
      ctx.drawImage(img, x, y, w, h, 0, 0, w * zoom, h * zoom);
      return c.toDataURL("image/png");
    },
    { src: dataUrl(src), x, y, w, h, zoom },
  );
  writeDataUrl(out, url);
}

/**
 * 在图的一块区域里找接近某个颜色的像素，返回它们的外接框和中心（像素坐标）。
 * 找不到时返回 null。
 */
export async function findColor(page, { src, rgb, tolerance = 40, region }) {
  return page.evaluate(
    async ({ src, rgb, tolerance, region }) => {
      const img = new Image();
      img.src = src;
      await img.decode();
      const r = region ?? { x: 0, y: 0, w: img.width, h: img.height };
      const c = new OffscreenCanvas(r.w, r.h);
      const ctx = c.getContext("2d");
      ctx.drawImage(img, r.x, r.y, r.w, r.h, 0, 0, r.w, r.h);
      const data = ctx.getImageData(0, 0, r.w, r.h).data;
      let minX = Infinity;
      let minY = Infinity;
      let maxX = -1;
      let maxY = -1;
      for (let y = 0; y < r.h; y++) {
        for (let x = 0; x < r.w; x++) {
          const i = (y * r.w + x) * 4;
          const d = Math.max(
            Math.abs(data[i] - rgb[0]),
            Math.abs(data[i + 1] - rgb[1]),
            Math.abs(data[i + 2] - rgb[2]),
          );
          if (d <= tolerance) {
            minX = Math.min(minX, x);
            minY = Math.min(minY, y);
            maxX = Math.max(maxX, x);
            maxY = Math.max(maxY, y);
          }
        }
      }
      if (maxX < 0) return null;
      return {
        box: { x: r.x + minX, y: r.y + minY, w: maxX - minX + 1, h: maxY - minY + 1 },
        center: { x: r.x + (minX + maxX + 1) / 2, y: r.y + (minY + maxY + 1) / 2 },
      };
    },
    { src: dataUrl(src), rgb, tolerance, region },
  );
}
