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
  it("双层渲染：水位细轨标百分比，满宽构成条按占已估比例显字；无未估算 MCP 时写「剩余」", () => {
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
    expect(screen.getByRole("img", { name: /占预算 20k 的 46%/ })).toBeInTheDocument();
    expect(screen.queryByText(/已估 /)).not.toBeInTheDocument();
    expect(screen.getByText("剩余 10.8k")).toBeInTheDocument();

    const blocks = screen.getAllByRole("button", { name: / · / });
    expect(blocks).toHaveLength(3);
    const lark = screen.getByRole("button", { name: /^lark · 6.4k/ });
    expect(lark).toHaveAttribute("data-glob", "true");
    expect(lark).toHaveAttribute("data-labeled", "true");
    expect(lark).toHaveTextContent("lark");

    // 400 / 9200 = 4.3% < 10%，不写字也不强撑标签内边距
    const harness = screen.getByRole("button", { name: /^harness/ });
    expect(harness).toHaveAttribute("data-ring", "warn");
    expect(harness).not.toHaveAttribute("data-labeled");
    expect(harness).toHaveTextContent("");

    // 有全局项且有待处理项时，按需图例均出现
    expect(screen.getByTitle("斜纹：全局层带进来的")).toBeInTheDocument();
    expect(screen.getByTitle("圆点：待处理（本地已改 · 可更新）")).toBeInTheDocument();

    fireEvent.click(harness);
    expect(onPick).toHaveBeenCalledWith("harness");
  });

  it("低占比与未估算 MCP 场景：写「已估 / 剩余 ≤」，大头仍显名，虚点块可点击定位", () => {
    const onPickUnestimated = vi.fn();
    const lowItems = [
      item("create-statusline 等 6 项", {
        kind: "skill",
        scope: "global",
        resident: 500,
        state: null,
      }),
      item("html 等 3 项", { kind: "skill", scope: "global", resident: 300, state: null }),
      item("tiny-skill", { kind: "skill", scope: "global", resident: 200, state: null }),
      item("mcp-1", { kind: "mcp", scope: "project", resident: null, state: null }),
      item("mcp-2", { kind: "mcp", scope: "project", resident: null, state: null }),
      item("mcp-3", { kind: "mcp", scope: "project", resident: null, state: null }),
    ];

    const { container } = render(
      <ContextMap
        items={lowItems}
        limit={20000}
        registered
        selectedId={undefined}
        hoverId={null}
        onHover={() => {}}
        onPick={() => {}}
        onPickUnestimated={onPickUnestimated}
        after={null}
      />,
    );

    expect(container.querySelector(".total")).toHaveTextContent("已估 1.0k");
    expect(screen.getByText(/\(5%\)/)).toBeInTheDocument();
    expect(screen.getByText("剩余 ≤19.0k")).toBeInTheDocument();
    expect(screen.getByText("MCP 3 个未估算")).toBeInTheDocument();

    // 占已估 50% 和 30% 的项直接显出名称与常驻
    const top = screen.getByRole("button", { name: /^create-statusline 等 6 项 · 0.5k/ });
    expect(top).toHaveAttribute("data-labeled", "true");
    expect(top).toHaveTextContent("create-statusline 等 6 项");
    expect(top).toHaveTextContent("0.5k");

    // 零待处理时不显示「待处理」图例
    expect(screen.queryByTitle("圆点：待处理（本地已改 · 可更新）")).not.toBeInTheDocument();

    // 点击末尾「MCP 3 项未估」虚点块触发定位回调
    const unestBtn = screen.getByRole("button", { name: /MCP 3 项未估/ });
    fireEvent.click(unestBtn);
    expect(onPickUnestimated).toHaveBeenCalledTimes(1);
  });

  it("仅有未估算 MCP（已估 0）时保留未估虚点块且隐藏全局图例", () => {
    const onlyMcp = [
      item("mcp-1", { kind: "mcp", scope: "project", resident: null, state: null }),
      item("mcp-2", { kind: "mcp", scope: "project", resident: null, state: null }),
    ];
    render(
      <ContextMap
        items={onlyMcp}
        limit={20000}
        registered
        selectedId={undefined}
        hoverId={null}
        onHover={() => {}}
        onPick={() => {}}
        after={null}
      />,
    );
    expect(screen.getByText("剩余 ≤20.0k")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /MCP 2 项未估/ })).toHaveTextContent(
      "MCP 2 项未估 · 点击定位",
    );
    expect(screen.queryByTitle("斜纹：全局层带进来的")).not.toBeInTheDocument();
  });

  it("既无计入常驻项也无未估算 MCP 时折叠构成条", () => {
    render(
      <ContextMap
        items={[item("off", { kind: "skill", scope: "project", resident: 300, counted: false })]}
        limit={20000}
        registered={false}
        selectedId={undefined}
        hoverId={null}
        onHover={() => {}}
        onPick={() => {}}
        after={null}
      />,
    );
    expect(screen.getByText("· 没有计入常驻的项")).toBeInTheDocument();
    expect(screen.queryByLabelText("已估常驻构成")).not.toBeInTheDocument();
  });

  it("超出预算时标出「超出」并带警告属性", () => {
    render(
      <ContextMap
        items={items}
        limit={5000}
        registered
        selectedId={undefined}
        hoverId={null}
        onHover={() => {}}
        onPick={() => {}}
        after={null}
      />,
    );
    const over = screen.getByText("超出 4.2k");
    expect(over).toHaveAttribute("data-over", "true");
  });

  it("有排进来的改动时写「· 同步后约 N」", () => {
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
