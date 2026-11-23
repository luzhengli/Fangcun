/**
 * 图标。路径照抄原型画板里的行内 SVG（16×16 视框，1.5 线宽，跟随文字颜色）。
 * 要加图标，从 `prototype/project/*.dc.html` 里取，不另画。
 */
import type { ReactNode } from "react";

const paths = {
  back: <path d="M10 3.5 5.5 8l4.5 4.5" />,
  forward: <path d="M6 3.5 10.5 8 6 12.5" />,
  sidebar: (
    <>
      <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="2" />
      <path d="M6 2.75v10.5" />
    </>
  ),
  inspector: (
    <>
      <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="2" />
      <path d="M10 2.75v10.5" />
    </>
  ),
  chevronDown: <path d="m4 6 4 4 4-4" />,
  check: <path d="m3.5 8.5 3 3 6-7" />,
  info: (
    <>
      <circle cx="8" cy="8" r="6" />
      <path d="M8 7.25v3.5" />
      <circle cx="8" cy="5.1" r=".55" fill="currentColor" stroke="none" />
    </>
  ),
  shield: <path d="M8 1.75 13.25 4v4c0 3-2.25 5.25-5.25 6.25C5 13.25 2.75 11 2.75 8V4z" />,
  search: (
    <>
      <circle cx="7" cy="7" r="4.5" />
      <path d="m10.5 10.5 3 3" />
    </>
  ),
  sync: (
    <>
      <path d="M13 6.5A5 5 0 0 0 4 4.5L2.5 6" />
      <path d="M2.5 3v3h3" />
      <path d="M3 9.5a5 5 0 0 0 9 2L13.5 10" />
      <path d="M13.5 13v-3h-3" />
    </>
  ),
  logo: (
    <>
      <rect x="1.75" y="1.75" width="12.5" height="12.5" rx="2.5" />
      <rect x="5.5" y="5.5" width="5" height="5" rx="1" fill="currentColor" stroke="none" />
    </>
  ),
  projects: (
    <path d="M2 4.5A1.5 1.5 0 0 1 3.5 3h2.6l1.4 1.5h5A1.5 1.5 0 0 1 14 6v5.5a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 2 11.5z" />
  ),
  library: (
    <>
      <path d="M2.5 5 8 2.5 13.5 5v6L8 13.5 2.5 11z" />
      <path d="M2.5 5 8 7.5 13.5 5M8 7.5v6" />
    </>
  ),
  bundles: (
    <>
      <path d="M8 2.5 14 5.5 8 8.5 2 5.5z" />
      <path d="M2 8.5l6 3 6-3" />
    </>
  ),
  global: (
    <>
      <circle cx="8" cy="8" r="5.5" />
      <path d="M2.5 8h11M8 2.5c1.6 1.7 2.4 3.5 2.4 5.5S9.6 11.8 8 13.5C6.4 11.8 5.6 10 5.6 8S6.4 4.2 8 2.5z" />
    </>
  ),
  activity: (
    <>
      <circle cx="8" cy="8" r="5.5" />
      <path d="M8 5v3l2 1.5" />
    </>
  ),
  checkup: <path d="M1.5 8h3l1.5-3.5 3 7 1.5-3.5h4" />,
  settings: (
    <>
      <circle cx="8" cy="8" r="2" />
      <path d="M8 1.75v1.5M8 12.75v1.5M1.75 8h1.5M12.75 8h1.5M3.6 3.6l1.05 1.05M11.35 11.35l1.05 1.05M3.6 12.4l1.05-1.05M11.35 4.65l1.05-1.05" />
      <circle cx="8" cy="8" r="4.5" />
    </>
  ),
  plus: <path d="M8 3v10M3 8h10" />,
  picked: (
    <>
      <rect x="2" y="2" width="12" height="12" rx="3" />
      <path d="m5 8.25 2 2 4-4.5" />
    </>
  ),
  chevron: <path d="M6 3.5 10.5 8 6 12.5" />,
  shieldCheck: (
    <>
      <path d="M8 1.75 13.25 4v4c0 3-2.25 5.25-5.25 6.25C5 13.25 2.75 11 2.75 8V4z" />
      <path d="m5.75 8 1.6 1.6 3-3.2" />
    </>
  ),
  general: (
    <>
      <circle cx="8" cy="8" r="2" />
      <circle cx="8" cy="8" r="4.75" />
      <path d="M8 1.5v1.75M8 12.75v1.75M1.5 8h1.75M12.75 8h1.75" />
    </>
  ),
  instruction: (
    <>
      <path d="M3.5 2.25h6.25l3 3V13a.75.75 0 0 1-.75.75h-8.5A.75.75 0 0 1 2.75 13V3a.75.75 0 0 1 .75-.75Z" />
      <path d="M9.75 2.25v3h3M5.25 8h5.5M5.25 10.5h3.75" />
    </>
  ),
  skill: (
    <>
      <path d="m8.75 1.75-5 7h4.25l-.75 5.5 5-7H8l.75-5.5Z" />
    </>
  ),
  mcp: (
    <>
      <circle cx="4.5" cy="4.5" r="1.75" />
      <circle cx="11.5" cy="4.5" r="1.75" />
      <circle cx="8" cy="11.75" r="1.75" />
      <path d="M6 5.5l1.5 4.5M10 5.5l-1.5 4.5M6.25 4.5h3.5" />
    </>
  ),
  copy: (
    <>
      <rect x="5.5" y="5.5" width="7.75" height="7.75" rx="1.5" />
      <path d="M10.5 5.5V3.5A1 1 0 0 0 9.5 2.5h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2" />
    </>
  ),
  close: (
    <>
      <path d="M4 4l8 8M12 4l-8 8" />
    </>
  ),
  arrowRight: (
    <>
      <path d="M3 8h9.5M8.75 4.25 12.5 8l-3.75 3.75" />
    </>
  ),
} satisfies Record<string, ReactNode>;

export type IconName = keyof typeof paths;

export function Icon({
  name,
  size = 16,
  strokeWidth = 1.5,
  className,
}: {
  name: IconName;
  size?: number;
  strokeWidth?: number;
  className?: string;
}) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
    >
      {paths[name]}
    </svg>
  );
}
