// 截图场景：画板的非默认状态（W-14）。
//
// 每个场景是一张画板加几步操作。操作按看得见的文字找元素，原型和应用里的文案一样，
// 所以同一组步骤两边都能走。`pnpm screens Main@codex` 截一个场景，不带参数时截全部画板和场景。
//
// 步骤：
//   { click: "文字" }       点第一个文字等于它的按钮、链接、页签、开关、菜单项、label；
//                           没有完全相等的，再找包含它的。
//   { click: "文字", nth: 1 }  第几个匹配（从 0 数）。
// focus：拼图里单独放大对比的区域，默认是检查器。找不到这个区域时只出整页拼图。

export const SCENES = {
  "Main@codex": {
    board: "Main",
    steps: [{ click: "Codex" }],
  },
  "Main@frontend-design": {
    board: "Main",
    steps: [{ click: "frontend-design" }],
  },
  "Main@unregistered": {
    board: "Main",
    steps: [{ click: "luyao-skills" }],
  },
  "Main@register-ask": {
    board: "Main",
    steps: [{ click: "luyao-skills" }, { click: "登记…" }],
    focus: '[role="dialog"]',
  },
  "Main@register-plan": {
    board: "Main",
    steps: [{ click: "luyao-skills" }, { click: "登记…" }, { click: "预览变更" }],
    focus: '[role="dialog"]',
  },
  "Main@registered": {
    board: "Main",
    steps: [
      { click: "luyao-skills" },
      { click: "登记…" },
      { click: "预览变更" },
      { click: "应用" },
    ],
  },
  "Main@sync-plan": {
    board: "Main",
    steps: [{ click: "同步" }],
    focus: '[role="dialog"]',
  },
  "Main@history": {
    board: "Main",
    steps: [{ click: "历史" }],
  },
  "Main@manifest": {
    board: "Main",
    steps: [{ click: "清单" }],
    focus: "main",
  },
  "Main@manifest-source": {
    board: "Main",
    steps: [{ click: "清单" }, { click: "fangcun.lock" }, { click: "源文件" }],
    focus: "main",
  },
  "Main@github": {
    board: "Main",
    steps: [{ click: "github" }],
  },
  "Settings@read-only": {
    board: "Settings",
    steps: [{ click: "只读模式" }],
    focus: "main",
  },
};

export const DEFAULT_FOCUS = 'aside[aria-label="检查器"]';

/**
 * 在页面里走一步。返回 null 表示成功，否则是一句说明哪一步走不通。
 * 在浏览器里执行，不依赖原型或应用的内部结构。
 */
export async function runStep(page, step) {
  if (step.click === undefined) return `不认识的步骤：${JSON.stringify(step)}`;
  const ok = await page.evaluate(
    ({ text, nth }) => {
      const pick = 'button, a, [role="tab"], [role="switch"], [role="menuitem"], label';
      const visible = (el) => {
        const r = el.getBoundingClientRect();
        return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== "hidden";
      };
      const textOf = (el) =>
        (el.getAttribute("aria-label") && !el.textContent.trim()
          ? el.getAttribute("aria-label")
          : el.textContent
        )
          .replace(/\s+/g, " ")
          .trim();
      const all = [...document.querySelectorAll(pick)].filter(visible);
      // 开关一类没有文字，用 aria-labelledby 指向的文字。
      const labelled = (el) => {
        const id = el.getAttribute("aria-labelledby");
        const ref = id && document.getElementById(id);
        return ref ? ref.textContent.trim() : "";
      };
      const exact = all.filter((el) => textOf(el) === text || labelled(el) === text);
      const loose = all.filter((el) => textOf(el).includes(text));
      const list = exact.length > 0 ? exact : loose;
      const el = list[nth ?? 0];
      if (!el) return false;
      el.click();
      return true;
    },
    { text: step.click, nth: step.nth },
  );
  if (!ok) return `找不到可点的「${step.click}」`;
  await page.waitForTimeout(150);
  return null;
}
