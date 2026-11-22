import { type ReactNode, useEffect, useId, useRef, useState } from "react";
import { create } from "zustand";
import c from "./controls.module.css";
import { Icon } from "./Icon";

/** 同一时刻只开一个气泡（6.1「ⓘ 说明钮」）。 */
const useOpenTip = create<{ id: string | null; set: (id: string | null) => void }>((set) => ({
  id: null,
  set: (id) => set({ id }),
}));

/**
 * ⓘ 说明钮（6.1、2026-11-21 人定）：说明性文字折进这里。
 * 点击或空格、回车弹出锚定在旁边的气泡（最宽 280px），再点一次、点别处或 Esc 关；悬停也显示。
 */
export function InfoTip({
  label,
  children,
  align = "left",
}: {
  /** 读屏念的名字，例如「地图说明」。 */
  label: string;
  children: ReactNode;
  /** 靠右的位置（检查器、窄栏）让气泡向左展开，免得出界。 */
  align?: "left" | "right";
}) {
  const id = useId();
  const bubbleId = `${id}-tip`;
  const pinned = useOpenTip((s) => s.id === id);
  const setOpen = useOpenTip((s) => s.set);
  const [hover, setHover] = useState(false);
  const wrap = useRef<HTMLSpanElement>(null);
  const shown = pinned || hover;

  useEffect(() => {
    if (!pinned) return;
    const onDown = (e: MouseEvent) => {
      if (!wrap.current?.contains(e.target as Node)) setOpen(null);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(null);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [pinned, setOpen]);

  return (
    <span ref={wrap} className={c.tipWrap}>
      <button
        type="button"
        className={c.tip}
        aria-label={label}
        aria-expanded={pinned}
        aria-controls={bubbleId}
        aria-describedby={shown ? bubbleId : undefined}
        onClick={() => setOpen(pinned ? null : id)}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        onFocus={() => setHover(true)}
        onBlur={() => setHover(false)}
      >
        <Icon name="info" size={13} strokeWidth={1.4} />
      </button>
      {shown && (
        <span id={bubbleId} role="tooltip" className={c.bubble} data-align={align}>
          {children}
        </span>
      )}
    </span>
  );
}
