import { useEffect, useId, useRef, useState } from "react";
import type { ToolId } from "@/ipc/bindings/ToolId";
import { useTools } from "@/ipc/queries";
import c from "./controls.module.css";
import { Icon } from "./Icon";

/**
 * 目标工具（6.3、6.10）：工具栏里只放图标加 ⌄ 的弹出按钮，悬停与读屏写「目标工具 · 名字」。
 * 菜单是「图标 + 全名」，当前项打勾，Esc 关。只列完整支持的工具。
 *
 * 图标贴近各家公开的标志，简化成 16px 线条（2026-11-21 人定「先用近官方版，发布前查」，R-07）。
 * 路径照抄 prototype/project/Main.dc.html。新工具按同样办法补；没有图标的工具显示名字首字母。
 */
export function ToolPicker({
  value,
  onChange,
}: {
  value: ToolId;
  onChange: (tool: ToolId) => void;
}) {
  const tools = useTools();
  const full = (tools.data ?? []).filter((t) => t.support === "full");
  const current = full.find((t) => t.id === value);
  const name = current?.name ?? value;
  const [open, setOpen] = useState(false);
  const menuId = useId();
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    // 菜单打开后焦点进第一项（当前项），关闭后回到按钮（6.1「键盘」）。
    menu.current?.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus();
    const onDown = (e: MouseEvent) => {
      if (!menu.current?.contains(e.target as Node) && !button.current?.contains(e.target as Node))
        setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const close = () => {
    setOpen(false);
    button.current?.focus();
  };

  return (
    <div className={c.menuWrap}>
      <button
        ref={button}
        type="button"
        className={`${c.b} ${c.quiet} ${c.toolButton}`}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        aria-label={`目标工具 · ${name}`}
        title={`目标工具 · ${name}`}
        onClick={() => setOpen(!open)}
      >
        <ToolIcon tool={value} name={name} />
        <span className={c.toolLabel}>{name}</span>
        <span className={c.chev}>
          <Icon name="chevronDown" size={10} strokeWidth={1.8} />
        </span>
      </button>
      {open && (
        <div
          ref={menu}
          id={menuId}
          role="menu"
          aria-label="目标工具"
          className={c.menu}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation();
              close();
            }
            if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              const items = [
                ...(menu.current?.querySelectorAll<HTMLButtonElement>("[role=menuitemradio]") ??
                  []),
              ];
              const at = items.indexOf(document.activeElement as HTMLButtonElement);
              const next = (at + (e.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
              items[next]?.focus();
            }
          }}
        >
          {full.map((tool) => (
            <button
              key={tool.id}
              type="button"
              role="menuitemradio"
              aria-checked={tool.id === value}
              className={c.menuItem}
              onClick={() => {
                onChange(tool.id);
                close();
              }}
            >
              <span aria-hidden="true" className={c.menuCheck}>
                {tool.id === value && <Icon name="check" size={14} strokeWidth={1.8} />}
              </span>
              <ToolIcon tool={tool.id} name={tool.name} />
              <span style={{ flex: 1 }}>{tool.name}</span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** 各工具的图标。Claude Code 是陶土橙的星芒，Codex 是跟随文字色的六瓣结。 */
export function ToolIcon({ tool, name }: { tool: ToolId; name: string }) {
  if (tool === "claude-code")
    return (
      <svg
        width="16"
        height="16"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.45"
        strokeLinecap="round"
        aria-hidden="true"
        className={c.ccIcon}
      >
        <path d="M8 8L14.63 8.93M8 8L12.33 11.39M8 8L10.58 14.40M8 8L7.19 13.74M8 8L4.06 13.04M8 8L3.09 9.99M8 8L1.27 7.05M8 8L3.59 4.55M8 8L5.53 1.88M8 8L8.82 2.16M8 8L12.25 2.56M8 8L13.01 5.98" />
      </svg>
    );
  if (tool === "codex")
    return (
      <svg
        width="16"
        height="16"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.15"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        {[0, 60, 120, 180, 240, 300].map((deg) => (
          <rect
            key={deg}
            x="6.50"
            y="2.20"
            width="4.4"
            height="8.0"
            rx="2.2"
            transform={`rotate(${deg} 8 8) rotate(12 8.70 6.20)`}
          />
        ))}
      </svg>
    );
  return (
    <span aria-hidden="true" style={{ width: 16, textAlign: "center", fontWeight: 600 }}>
      {name.charAt(0)}
    </span>
  );
}
