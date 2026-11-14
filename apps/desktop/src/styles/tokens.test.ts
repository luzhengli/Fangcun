import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/** 把一段 CSS 声明读成 token → 取值。取值里没有分号。 */
function declarations(block: string): Record<string, string> {
  const tokens: Record<string, string> = {};
  for (const declaration of block.split(";")) {
    const colon = declaration.indexOf(":");
    if (colon === -1) continue;
    tokens[declaration.slice(0, colon).trim()] = declaration.slice(colon + 1).trim();
  }
  return tokens;
}

function read(relative: string): string {
  return readFileSync(fileURLToPath(new URL(relative, import.meta.url)), "utf8");
}

function capture(source: string, pattern: RegExp): string {
  const block = pattern.exec(source)?.[1];
  if (block === undefined) throw new Error(`没有找到 ${pattern}`);
  return block;
}

describe("墨格 token", () => {
  const prototype = read("../../../../prototype/project/DesignSystem.dc.html");
  const app = read("./tokens.css");

  it("浅色与原型的 .fc 逐项一致", () => {
    const expected = declarations(capture(prototype, /\.fc\{([^}]*)\}/));
    const actual = declarations(capture(app, /^:root \{([^}]*)\}/m));
    expect(actual).toEqual(expected);
  });

  it("深色与原型的 .fc.dark 逐项一致", () => {
    const expected = declarations(capture(prototype, /\.fc\.dark\{([^}]*)\}/));
    const actual = declarations(
      capture(app, /prefers-color-scheme: dark\) \{\s*:root \{([^}]*)\}/),
    );
    expect(actual).toEqual(expected);
  });
});
