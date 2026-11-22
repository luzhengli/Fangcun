import { useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { useLocation, useMatch, useNavigate } from "react-router";
import type { AssetHit } from "@/ipc/bindings/AssetHit";
import { kernel } from "@/ipc/commands";
import { useEffectiveContext, useProjects, useRescan } from "@/ipc/queries";
import { projectPath } from "@/shell/nav";
import { usePalette } from "@/state/palette";
import { usePlan } from "@/state/plan";
import { useUi } from "@/state/ui";
import styles from "./CommandPalette.module.css";
import c from "./controls.module.css";
import { Icon } from "./Icon";
import { useDialogFocus } from "./useDialogFocus";

interface Entry {
  key: string;
  kind: "资产" | "项目" | "动作";
  name: string;
  hint: string;
  go: () => void;
}

const KIND_NAME = { skill: "Skill", mcp: "MCP", instruction: "指令" } as const;

/** 搜索词停下 150ms 再问内核，免得每敲一个字扫一次资产库。 */
function useDebounced(value: string, ms: number): string {
  const [out, setOut] = useState(value);
  useEffect(() => {
    const t = window.setTimeout(() => setOut(value), ms);
    return () => window.clearTimeout(t);
  }, [value, ms]);
  return out;
}

/**
 * 命令面板 `⌘K`（6.1、6.9）：搜索资产、项目、动作。`↑↓` 选择、`↩` 打开、`Esc` 关闭。
 *
 * 在项目页里，选中一项本项目已有的资产就定位到它；没有的，回车加进这个项目的清单，
 * 弹出变更计划（`plan_add`），全程不离开键盘。skills.sh 只在点了「在 skills.sh 搜索」后才联网，
 * 只发搜索词（F2）。
 */
export function CommandPalette() {
  const { open, show, hide } = usePalette();

  // ⌘K 全局打开。弹窗开着（变更计划、登记）时不抢。
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        if (!open && document.querySelector('[role="dialog"]')) return;
        e.preventDefault();
        if (open) hide();
        else show();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, show, hide]);

  if (!open) return null;
  return <Panel onClose={hide} />;
}

function Panel({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate();
  const location = useLocation();
  const match = useMatch("/projects/:projectId");
  const root = match?.params.projectId ? decodeURIComponent(match.params.projectId) : undefined;
  const tool = useUi((s) => s.tool);
  const projects = useProjects();
  const context = useEffectiveContext(root, tool);
  const rescan = useRescan();
  const project = projects.data?.find((p) => p.id === root);
  const registered = project?.registered ?? false;

  const [query, setQuery] = useState("");
  const [index, setIndex] = useState(0);
  const [market, setMarket] = useState(false);
  const q = useDebounced(query.trim(), 150);
  const hits = useQuery({
    queryKey: ["search-assets", q, market],
    queryFn: () => kernel.searchAssets(q, market),
    enabled: q.length > 0,
    staleTime: 30_000,
  });
  const input = useRef<HTMLInputElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  // 打开时焦点进输入框（面板里第一个可聚焦的），关闭后回到打开前的地方。
  useDialogFocus(panel, true);

  const items = context.data?.items ?? [];
  const go = (to: string) => {
    onClose();
    navigate(to);
  };
  const add = (hit: AssetHit) => {
    if (!root || !hit.add) return;
    const spec = hit.add;
    onClose();
    void usePlan.getState().preview({
      load: () => kernel.planAdd(root, spec),
      title: (plan) =>
        `把 ${hit.name} 加入 ${project?.name ?? "这个项目"} · ${plan.changes.length} 处变更`,
      sub: () => "改清单、更新锁并同步 · 应用前留快照，可撤销",
      done: () => `已把 ${hit.name} 加入 ${project?.name ?? "项目"} · 已留快照`,
    });
  };

  const assetEntry = (hit: AssetHit): Entry => {
    const here = items.find((i) => i.name === hit.name);
    const source = hit.origin === "market" ? "skills.sh" : (hit.source ?? "来源未知");
    let hint = `${KIND_NAME[hit.kind]} · ${source}`;
    let action = () => go(`/library?asset=${encodeURIComponent(hit.name)}`);
    if (root && here) {
      hint = `${KIND_NAME[hit.kind]} · 本项目已有`;
      action = () => go(`${projectPath(root)}?asset=${encodeURIComponent(hit.name)}`);
    } else if (root && registered && hit.add) {
      hint = `加入 ${project?.name}${hit.installs ? ` · ${hit.installs} 次安装` : ` · ${source}`}`;
      action = () => add(hit);
    } else if (hit.origin === "market") {
      hint = `skills.sh${hit.installs ? ` · ${hit.installs} 次安装` : ""} · 先打开一个已登记的项目再加入`;
      action = () => undefined;
    }
    return {
      key: `a:${hit.origin}:${hit.name}:${hit.source ?? ""}`,
      kind: "资产",
      name: hit.name,
      hint,
      go: action,
    };
  };

  const needle = query.trim().toLowerCase();
  const matches = (e: Entry) => !needle || `${e.name} ${e.hint}`.toLowerCase().includes(needle);

  const actions: Entry[] = [];
  if (root && project) {
    if (registered) {
      actions.push({
        key: "sync",
        kind: "动作",
        name: `同步 ${project.name}`,
        hint: "按清单与锁物化，先出变更计划",
        go: () => go(`${projectPath(root)}?do=sync`),
      });
      actions.push({
        key: "manifest",
        kind: "动作",
        name: "查看清单 fangcun.toml",
        hint: "",
        go: () => go(`${projectPath(root)}?tab=manifest`),
      });
      actions.push({
        key: "history",
        kind: "动作",
        name: "查看历史与回滚",
        hint: "",
        go: () => go(`${projectPath(root)}?tab=history`),
      });
    }
  }
  actions.push(
    {
      key: "checkup",
      kind: "动作",
      name: "重新体检",
      hint: "只读扫描全盘",
      go: () => {
        rescan.mutate();
        go("/checkup");
      },
    },
    {
      key: "upstream",
      kind: "动作",
      name: "检查更新",
      hint: "向来源查有没有新版本",
      go: () => go("/activity?check=1"),
    },
    { key: "library", kind: "动作", name: "打开资产库", hint: "", go: () => go("/library") },
    { key: "global", kind: "动作", name: "打开全局", hint: "瘦身", go: () => go("/global") },
    {
      key: "activity",
      kind: "动作",
      name: "打开活动",
      hint: "时间线与回滚",
      go: () => go("/activity"),
    },
  );
  const projectEntries: Entry[] = (projects.data ?? [])
    .filter((p) => p.id !== root)
    .map((p) => ({
      key: `p:${p.id}`,
      kind: "项目" as const,
      name: p.name,
      hint: p.registered ? p.path : `${p.path} · 未登记`,
      go: () => go(projectPath(p.id)),
    }));

  const assets = (hits.data ?? []).map(assetEntry);
  // 搜索词也能直接对上本项目已有的项（例如来源未知、不在资产库里的）。
  const local: Entry[] =
    root && needle
      ? items
          .filter(
            (i) => i.name.toLowerCase().includes(needle) && !assets.some((a) => a.name === i.name),
          )
          .slice(0, 6)
          .map((i) => ({
            key: `i:${i.id}`,
            kind: "资产" as const,
            name: i.name,
            hint: "本项目已有",
            go: () => go(`${projectPath(root)}?asset=${encodeURIComponent(i.name)}`),
          }))
      : [];
  const marketEntry: Entry[] =
    needle && !market
      ? [
          {
            key: "market",
            kind: "动作",
            name: `在 skills.sh 搜索「${query.trim()}」`,
            hint: "联网，只发搜索词",
            go: () => {
              setMarket(true);
              input.current?.focus();
            },
          },
        ]
      : [];
  const results = [
    ...assets,
    ...local,
    ...actions.filter(matches),
    ...projectEntries.filter(matches).slice(0, needle ? 8 : 6),
    ...marketEntry,
  ];
  const active = Math.min(index, Math.max(0, results.length - 1));

  // 换页面时关掉（例如点了侧栏）。
  const [path] = useState(location.pathname);
  useEffect(() => {
    if (location.pathname !== path) onClose();
  }, [location.pathname, path, onClose]);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      setIndex(Math.min(results.length - 1, active + 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setIndex(Math.max(0, active - 1));
    } else if (e.key === "Enter" && !e.nativeEvent.isComposing) {
      e.preventDefault();
      results[active]?.go();
    }
  };

  const searching = hits.isFetching && q.length > 0;
  return (
    <div className={styles.veil}>
      <button
        type="button"
        tabIndex={-1}
        aria-label="关闭命令面板"
        className={styles.backdrop}
        onClick={onClose}
      />
      <div
        role="dialog"
        aria-modal="true"
        aria-label="命令面板"
        ref={panel}
        className={styles.panel}
        onKeyDown={onKey}
      >
        <div className={styles.head}>
          <Icon name="search" size={16} strokeWidth={1.6} />
          <label htmlFor="palette-q" className={styles.srOnly}>
            搜索资产、项目、动作
          </label>
          <input
            id="palette-q"
            ref={input}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setIndex(0);
              setMarket(false);
            }}
            placeholder="搜索资产、项目、动作"
            role="combobox"
            aria-expanded="true"
            aria-controls="palette-results"
            aria-activedescendant={results[active] ? `pal-${active}` : undefined}
            autoComplete="off"
            spellCheck={false}
            className={styles.input}
          />
          <span className={c.kbd}>esc</span>
        </div>
        <div id="palette-results" role="listbox" className={styles.results}>
          {results.length === marketEntry.length && !searching && (
            <div className={styles.empty}>没有匹配的资产或动作</div>
          )}
          {results.map((r, i) => (
            <button
              key={r.key}
              id={`pal-${i}`}
              type="button"
              role="option"
              aria-selected={i === active}
              className={styles.row}
              onMouseMove={() => i !== active && setIndex(i)}
              onClick={r.go}
            >
              <span className={c.tag}>{r.kind}</span>
              <span className={styles.name}>{r.name}</span>
              <span className={styles.hint}>{r.hint}</span>
            </button>
          ))}
          {hits.isError && <div className={styles.error}>{hits.error.message}</div>}
        </div>
        <div className={styles.foot}>
          <span>
            <span className={c.kbd}>↑↓</span> 选择
          </span>
          <span>
            <span className={c.kbd}>↩</span> 打开
          </span>
          <span>
            <span className={c.kbd}>esc</span> 关闭
          </span>
          <span className={styles.flex} />
          <span>
            {searching ? "正在搜索…" : root ? (registered ? "项目内" : "未登记 · 只读") : ""}
          </span>
        </div>
      </div>
    </div>
  );
}
