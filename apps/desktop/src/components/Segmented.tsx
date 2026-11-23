import {
  type ReactNode,
  type RefObject,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import c from "./controls.module.css";

export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  count?: number;
  title?: string;
}

interface PillBox {
  left: number;
  width: number;
}

function measurePill(el: HTMLElement | null, role: "group" | "tablist"): PillBox | null {
  if (!el) return null;
  const selector =
    role === "tablist" ? 'button[aria-selected="true"]' : 'button[aria-pressed="true"]';
  const active = el.querySelector<HTMLButtonElement>(selector);
  if (!active || active.offsetWidth <= 0) return null;
  return { left: active.offsetLeft, width: active.offsetWidth };
}

/**
 * 滑动分段控件：选中项切换时，白底胶囊通过 CSS transform 平滑滑入。
 * 在无几何尺寸的单元测试（jsdom offsetWidth === 0）中自动回退为静态高亮，不破坏无障碍属性。
 */
export function Segmented<T extends string>({
  label,
  options,
  value,
  onChange,
  role = "group",
  className,
}: {
  label: string;
  options: readonly SegmentedOption<T>[];
  value: T;
  onChange: (value: T) => void;
  /** fieldset 分组筛选（aria-pressed）或页面主视图标签页（tablist + aria-selected）。 */
  role?: "group" | "tablist";
  className?: string;
}) {
  const wrapRef = useRef<HTMLElement>(null);
  const [pill, setPill] = useState<PillBox | null>(null);
  const key = options.map((o) => o.value).join("\u0000");

  useLayoutEffect(() => {
    void value;
    void key;
    setPill(measurePill(wrapRef.current, role));
  }, [value, key, role]);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => setPill(measurePill(el, role)));
    ro.observe(el);
    return () => ro.disconnect();
  }, [role]);

  const sharedClass = `${c.seg}${className ? ` ${className}` : ""}`;
  const ready = pill !== null;

  const buttons = options.map((opt) => {
    const active = opt.value === value;
    const aria =
      role === "tablist"
        ? { role: "tab" as const, "aria-selected": active }
        : { "aria-pressed": active };
    return (
      <button
        key={opt.value}
        type="button"
        title={opt.title}
        onClick={() => onChange(opt.value)}
        {...aria}
      >
        {opt.label}
        {opt.count !== undefined && <span className={c.count}>{opt.count}</span>}
      </button>
    );
  });

  const indicator = ready && (
    <span
      aria-hidden="true"
      className={c.segPill}
      style={{
        width: `${pill.width}px`,
        transform: `translateX(${pill.left}px)`,
      }}
    />
  );

  if (role === "tablist") {
    return (
      <div
        ref={wrapRef as RefObject<HTMLDivElement>}
        role="tablist"
        aria-label={label}
        className={sharedClass}
        data-ready={ready}
      >
        {indicator}
        {buttons}
      </div>
    );
  }

  return (
    <fieldset
      ref={wrapRef as RefObject<HTMLFieldSetElement>}
      aria-label={label}
      className={sharedClass}
      data-ready={ready}
    >
      {indicator}
      {buttons}
    </fieldset>
  );
}
