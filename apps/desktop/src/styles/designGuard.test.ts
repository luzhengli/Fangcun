import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const SRC_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const UI_DIRS = ["components", "pages", "shell", "state"] as const;
const TERM_DIRS = [...UI_DIRS, "ipc/fixtures"] as const;

const FORBIDDEN_TERMS = [
  "物化",
  "收编",
  "纳管",
  "已漂移",
  "处漂移",
  "回流到",
  "瘦身托盘",
  "加入托盘",
] as const;

function walk(dir: string, acc: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      walk(full, acc);
    } else {
      acc.push(full);
    }
  }
  return acc;
}

/** 仅剥离块注释与行注释，保留字符串字面量，便于检测界面代码里的禁用词与硬编码色值。 */
function stripComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\/|(^|[^:])\/\/[^\n]*/gm, "$1");
}

describe("设计系统与术语规范自动化守卫（A-08 / P-16）", () => {
  it("P-16：界面、组件、状态与示例数据文案不得包含工程黑话", () => {
    const violations: string[] = [];
    for (const dir of TERM_DIRS) {
      const files = walk(join(SRC_ROOT, dir)).filter(
        (f) => /\.(ts|tsx)$/.test(f) && !/\.test\.(ts|tsx)$/.test(f),
      );
      for (const file of files) {
        const clean = stripComments(readFileSync(file, "utf8"));
        const rel = relative(SRC_ROOT, file);
        for (const term of FORBIDDEN_TERMS) {
          if (clean.includes(term)) {
            violations.push(`${rel}: 包含禁用词「${term}」`);
          }
        }
      }
    }
    expect(violations).toEqual([]);
  });

  it("A-08：界面样式表与组件代码中不得硬编码 #hex 或 rgb()/rgba() 色值", () => {
    const violations: string[] = [];
    const colorPattern = /#[0-9a-fA-F]{3,8}\b|rgba?\(/gi;
    for (const dir of UI_DIRS) {
      const files = walk(join(SRC_ROOT, dir)).filter(
        (f) => /\.(css|ts|tsx)$/.test(f) && !/\.test\.(ts|tsx)$/.test(f),
      );
      for (const file of files) {
        const clean = stripComments(readFileSync(file, "utf8"));
        const rel = relative(SRC_ROOT, file);
        const matches = clean.match(colorPattern);
        if (matches && matches.length > 0) {
          violations.push(`${rel}: 存在硬编码色值 ${matches.join(", ")}`);
        }
      }
    }
    expect(violations).toEqual([]);
  });
});
