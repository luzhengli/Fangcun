/**
 * 图标。路径照抄原型画板里的行内 SVG（16×16 视框，1.5 线宽，跟随文字颜色）。
 * 要加图标，从 `prototype/project/*.dc.html` 里取，不另画。
 */
import type { ReactNode } from "react";

const paths = {
  back: <path d="M13 8H3M7 4 3 8l4 4" />,
  forward: <path d="M3 8h10M9 4l4 4-4 4" />,
  sidebar: (
    <>
      <rect x="2" y="2.5" width="12" height="11" rx="2" />
      <path d="M6.5 2.5v11" />
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
} satisfies Record<string, ReactNode>;

export type IconName = keyof typeof paths;

export function Icon({ name, size = 16 }: { name: IconName; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths[name]}
    </svg>
  );
}
