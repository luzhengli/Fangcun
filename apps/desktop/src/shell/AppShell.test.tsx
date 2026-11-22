import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
import { FEATURES } from "@/app/features";
import { routes } from "@/app/router";
import { useUi } from "@/state/ui";

function renderApp(path = "/") {
  const router = createMemoryRouter(routes, { initialEntries: [path] });
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

describe("窗口骨架（6.1）", () => {
  beforeEach(() => {
    useUi.setState({ sidebarCollapsed: false });
  });

  it("侧栏导航顺序是项目、资产库、（套装）、全局、活动；底部是设置和重新体检", () => {
    renderApp();
    const nav = screen.getByRole("navigation", { name: "主导航" });
    const labels = within(nav)
      .getAllByRole("link")
      .map((link) => link.textContent);
    // v0.1 套装隐藏（9.3、M1-35），FEATURES.bundles 打开时才有「套装」。
    const expected = FEATURES.bundles
      ? ["项目", "资产库", "套装", "全局", "活动"]
      : ["项目", "资产库", "全局", "活动"];
    expect(labels.slice(0, expected.length)).toEqual(expected);
    expect(within(nav).getByRole("link", { name: /^设置/ })).toHaveAttribute("href", "/settings");
    expect(within(nav).getByRole("button", { name: "重新体检" })).toBeInTheDocument();
  });

  it("启动先进体检报告（6.2）", () => {
    const router = renderApp("/");
    expect(router.state.location.pathname).toBe("/checkup");
  });

  it("没有历史时后退与前进置灰", () => {
    renderApp("/");
    expect(screen.getByRole("button", { name: "后退" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "前进" })).toBeDisabled();
  });

  it("侧栏底部写着「本地优先 · 零遥测」；收起侧栏时整个藏起，工具栏里出现「显示侧栏」", () => {
    renderApp();
    expect(screen.getByText("本地优先 · 零遥测")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "隐藏侧栏" }));
    expect(screen.queryByRole("navigation", { name: "主导航" })).not.toBeInTheDocument();
    const show = screen.getByRole("button", { name: "显示侧栏" });
    expect(show).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(show);
    expect(screen.getByRole("navigation", { name: "主导航" })).toBeInTheDocument();
  });

  it("页面标题在统一工具栏里", () => {
    renderApp("/activity");
    const toolbar = screen.getByRole("toolbar", { name: "窗口工具栏" });
    expect(within(toolbar).getByRole("heading", { level: 1, name: "活动" })).toBeInTheDocument();
    expect(
      within(toolbar).getByText("所有项目的落盘记录 · 每次写入都留快照，可一键回滚"),
    ).toBeInTheDocument();
  });

  it.each([
    ["/checkup", "体检报告"],
    ["/projects", "项目"],
    ["/library", "资产库"],
    ["/bundles", "套装"],
    ["/global", "全局"],
    ["/activity", "活动"],
  ])("%s 能打开", (path, title) => {
    renderApp(path);
    expect(screen.getByRole("heading", { level: 1, name: title })).toBeInTheDocument();
  });
});
