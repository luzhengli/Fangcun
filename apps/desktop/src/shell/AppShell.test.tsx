import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it } from "vitest";
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

  it("侧栏有五个导航项，顺序是项目、资产库、套装、全局、活动", () => {
    renderApp();
    const nav = screen.getByRole("navigation", { name: "主导航" });
    const labels = within(nav)
      .getAllByRole("link")
      .map((link) => link.textContent);
    expect(labels).toEqual(["项目", "资产库", "套装", "全局", "活动"]);
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

  it("侧栏底部写着「本地优先 · 零遥测」，折叠后不显示", () => {
    renderApp();
    expect(screen.getByText("本地优先 · 零遥测")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "折叠侧栏" }));
    expect(screen.queryByText("本地优先 · 零遥测")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "展开侧栏" })).toHaveAttribute(
      "aria-expanded",
      "false",
    );
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
