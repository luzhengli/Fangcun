import { act, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { usePresence, useRetainedValue } from "./usePresence";

function PresenceProbe({
  open,
  exitMs = 100,
  label,
}: {
  open: boolean;
  exitMs?: number;
  label: string | null;
}) {
  const presence = usePresence(open, exitMs);
  const retained = useRetainedValue(label);
  if (!presence.mounted) return null;
  return (
    <div
      data-testid="probe"
      data-entering={String(presence.entering)}
      data-exiting={String(presence.exiting)}
    >
      {retained}
    </div>
  );
}

describe("usePresence 与 useRetainedValue", () => {
  const originalUa = navigator.userAgent;
  const originalMatchMedia = window.matchMedia;

  afterEach(() => {
    Object.defineProperty(navigator, "userAgent", {
      value: originalUa,
      configurable: true,
    });
    Object.defineProperty(window, "matchMedia", {
      value: originalMatchMedia,
      configurable: true,
    });
    vi.useRealTimers();
  });

  function mockDesktopEnv(reducedMotion: boolean) {
    Object.defineProperty(navigator, "userAgent", {
      value: "Mozilla/5.0 (Macintosh; Intel Mac OS X) AppleWebKit/605.1.15",
      configurable: true,
    });
    Object.defineProperty(window, "matchMedia", {
      value: vi.fn().mockImplementation((query: string) => ({
        matches: query.includes("prefers-reduced-motion") ? reducedMotion : false,
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
      configurable: true,
    });
  }

  it("在 jsdom 默认环境下关闭时 0ms 同步卸载", () => {
    const { rerender } = render(<PresenceProbe open label="初始内容" />);
    expect(screen.getByTestId("probe")).toHaveTextContent("初始内容");
    rerender(<PresenceProbe open={false} label={null} />);
    expect(screen.queryByTestId("probe")).not.toBeInTheDocument();
  });

  it("在减少动态效果（prefers-reduced-motion: reduce）下 0ms 同步卸载", () => {
    mockDesktopEnv(true);
    const { rerender } = render(<PresenceProbe open label="内容" />);
    expect(screen.getByTestId("probe")).toBeInTheDocument();
    rerender(<PresenceProbe open={false} label={null} />);
    expect(screen.queryByTestId("probe")).not.toBeInTheDocument();
  });

  it("桌面环境下首次挂载不标 entering；关闭先进入退场并保留快照，到期卸载且可中途重开打断", () => {
    vi.useFakeTimers();
    mockDesktopEnv(false);

    const { rerender } = render(<PresenceProbe open exitMs={100} label="计划 A" />);
    const initial = screen.getByTestId("probe");
    expect(initial).toHaveAttribute("data-entering", "false");
    expect(initial).toHaveAttribute("data-exiting", "false");

    // 触发退场：保留上一份文案并在 100ms 内保持挂载
    rerender(<PresenceProbe open={false} exitMs={100} label={null} />);
    const exiting = screen.getByTestId("probe");
    expect(exiting).toHaveAttribute("data-exiting", "true");
    expect(exiting).toHaveTextContent("计划 A");

    // 中途 50ms 时重新打开：立刻打断退场定时器并切入 entering
    act(() => {
      vi.advanceTimersByTime(50);
    });
    rerender(<PresenceProbe open exitMs={100} label="计划 B" />);
    const reopened = screen.getByTestId("probe");
    expect(reopened).toHaveAttribute("data-exiting", "false");
    expect(reopened).toHaveAttribute("data-entering", "true");
    expect(reopened).toHaveTextContent("计划 B");

    // 再推进 100ms：原退场定时器已被清除，面板仍保持挂载且 entering 自动收敛为 false
    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(screen.getByTestId("probe")).toHaveAttribute("data-entering", "false");

    // 再次关闭并等待完整 100ms：完成卸载
    rerender(<PresenceProbe open={false} exitMs={100} label={null} />);
    expect(screen.getByTestId("probe")).toHaveAttribute("data-exiting", "true");
    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(screen.queryByTestId("probe")).not.toBeInTheDocument();
  });
});
