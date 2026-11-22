import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { describe, expect, it } from "vitest";
import { routes } from "@/app/router";
import { usePalette } from "@/state/palette";

function renderApp(path: string) {
  const router = createMemoryRouter(routes, { initialEntries: [path] });
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return router;
}

describe("命令面板 ⌘K（6.1）", () => {
  it("⌘K 打开，输入筛选动作，↩ 打开并关掉面板", () => {
    const router = renderApp("/global");
    fireEvent.keyDown(window, { key: "k", metaKey: true });
    const dialog = screen.getByRole("dialog", { name: "命令面板" });
    const input = within(dialog).getByRole("combobox");
    expect(input).toHaveFocus();
    expect(within(dialog).getByText("↑↓")).toBeInTheDocument();
    fireEvent.change(input, { target: { value: "活动" } });
    expect(within(dialog).getByRole("option", { name: /打开活动/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    fireEvent.keyDown(input, { key: "Enter" });
    expect(router.state.location.pathname).toBe("/activity");
    expect(screen.queryByRole("dialog", { name: "命令面板" })).not.toBeInTheDocument();
  });

  it("没有匹配时写「没有匹配的资产或动作」，Esc 关闭", () => {
    renderApp("/global");
    usePalette.getState().show();
    return Promise.resolve().then(async () => {
      const dialog = await screen.findByRole("dialog", { name: "命令面板" });
      const input = within(dialog).getByRole("combobox");
      fireEvent.change(input, { target: { value: "zzzz-没有这一项" } });
      // 资产搜索要等 150ms 再问内核，动作和项目当场筛完；联网搜索只是一个可选的动作。
      expect(within(dialog).getByText("没有匹配的资产或动作")).toBeInTheDocument();
      expect(within(dialog).getByRole("option", { name: /在 skills.sh 搜索/ })).toBeInTheDocument();
      fireEvent.keyDown(input, { key: "Escape" });
      expect(screen.queryByRole("dialog", { name: "命令面板" })).not.toBeInTheDocument();
    });
  });
});
