import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { InfoTip } from "./InfoTip";
import { ToolPicker } from "./ToolPicker";

function withQuery(node: React.ReactNode) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  client.setQueryData(
    ["tools"],
    [
      { id: "claude-code", name: "Claude Code", support: "full" },
      { id: "codex", name: "Codex", support: "full" },
      { id: "windsurf", name: "Windsurf", support: "scan-only" },
    ],
  );
  return render(<QueryClientProvider client={client}>{node}</QueryClientProvider>);
}

describe("目标工具（6.3）", () => {
  it("图标旁显示当前工具名，读屏写「目标工具 · 名字」；菜单只列完整支持的工具，当前项打勾，Esc 关", () => {
    const onChange = vi.fn();
    withQuery(<ToolPicker value="claude-code" onChange={onChange} />);
    const button = screen.getByRole("button", { name: "目标工具 · Claude Code" });
    expect(button).toHaveTextContent("Claude Code");
    fireEvent.click(button);
    const items = screen.getAllByRole("menuitemradio");
    expect(items.map((i) => i.textContent)).toEqual(["Claude Code", "Codex"]);
    expect(items[0]).toHaveAttribute("aria-checked", "true");
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    fireEvent.click(button);
    fireEvent.click(screen.getByRole("menuitemradio", { name: "Codex" }));
    expect(onChange).toHaveBeenCalledWith("codex");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});

describe("ⓘ 说明钮（6.1）", () => {
  it("点击弹出气泡，再点一次关；Esc 也关；同一时刻只开一个", () => {
    render(
      <>
        <InfoTip label="甲">甲的说明</InfoTip>
        <InfoTip label="乙">乙的说明</InfoTip>
      </>,
    );
    const a = screen.getByRole("button", { name: "甲" });
    const b = screen.getByRole("button", { name: "乙" });
    fireEvent.click(a);
    expect(screen.getByRole("tooltip")).toHaveTextContent("甲的说明");
    fireEvent.click(b);
    expect(screen.getAllByRole("tooltip")).toHaveLength(1);
    expect(screen.getByRole("tooltip")).toHaveTextContent("乙的说明");
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  });
});
