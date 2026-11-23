import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { EffectiveItem } from "@/ipc/bindings/EffectiveItem";
import { ContextMap } from "./ContextMap";

const base = {
  version: null,
  bundle: null,
  label: "正本",
  counted: true,
  members: [],
  command: null,
  reason: "",
  triggers: null,
  files: [],
  secrets: [],
  recent: [],
  drift: null,
  update: null,
  overriddenBy: null,
  disable: null,
};

function item(id: string, over: Partial<EffectiveItem>): EffectiveItem {
  return { ...base, id, name: id, location: id, ...over } as unknown as EffectiveItem;
}

const items = [
  item("AGENTS.md", { kind: "instruction", scope: "project", resident: 2400, state: null }),
  item("lark", { kind: "skill", scope: "global", resident: 6400, state: null }),
  item("harness", { kind: "skill", scope: "project", resident: 400, state: "drifted" }),
  item("off", { kind: "skill", scope: "project", resident: 300, counted: false, state: null }),
];

describe("上下文地图（6.3）", () => {
  it("只画计入常驻的项；全局斜纹、待处理圆点；小于 9% 的块不写字；剩余按预算算", () => {
    const onPick = vi.fn();
    render(
      <ContextMap
        items={items}
        limit={20000}
        registered
        selectedId={undefined}
        hoverId={null}
        onHover={() => {}}
        onPick={onPick}
        after={null}
      />,
    );
    const blocks = screen.getAllByRole("button", { name: / · / });
    expect(blocks).toHaveLength(3);
    const lark = screen.getByRole("button", { name: /^lark · 6.4k/ });
    expect(lark).toHaveAttribute("data-glob", "true");
    expect(lark).toHaveTextContent("lark");
    const harness = screen.getByRole("button", { name: /^harness/ });
    expect(harness).toHaveAttribute("data-ring", "warn");
    expect(harness).toHaveTextContent("");
    expect(screen.getAllByText(/^剩余 /).length).toBeGreaterThan(0);
    fireEvent.click(harness);
    expect(onPick).toHaveBeenCalledWith("harness");
  });

  it("有排进来的改动时写「同步后约 N」", () => {
    render(
      <ContextMap
        items={items}
        limit={20000}
        registered
        selectedId={undefined}
        hoverId={null}
        onHover={() => {}}
        onPick={() => {}}
        after={2800}
      />,
    );
    expect(screen.getByText("· 同步后约 2.8k")).toBeInTheDocument();
  });

  it("待停用项传入 stagedDisabledIds 时呈现虚线释放预演态", () => {
    render(
      <ContextMap
        items={items}
        limit={20000}
        registered
        selectedId={undefined}
        hoverId={null}
        onHover={() => {}}
        onPick={() => {}}
        stagedDisabledIds={new Set(["lark"])}
        after={2800}
      />,
    );
    expect(screen.getByRole("button", { name: /^lark · 6.4k/ })).toHaveAttribute(
      "data-staged",
      "true",
    );
  });
});
