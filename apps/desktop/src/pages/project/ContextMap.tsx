import { useState } from "react";
import { InfoTip } from "@/components/InfoTip";
import { Segmented } from "@/components/Segmented";
import { formatK, formatLimit } from "@/components/tone";
import type { AssetKind } from "@/ipc/bindings/AssetKind";
import type { EffectiveItem } from "@/ipc/bindings/EffectiveItem";
import type { Scope } from "@/ipc/bindings/Scope";
import styles from "./ContextMap.module.css";

type Mode = "kind" | "layer";

const KIND_ORDER: AssetKind[] = ["instruction", "skill", "mcp"];
const LAYER_ORDER: Scope[] = ["global", "bundle", "project", "personal"];
const KIND_NAME: Record<AssetKind, string> = { instruction: "指令", skill: "Skill", mcp: "MCP" };
const LAYER_NAME: Record<Scope, string> = {
  global: "全局",
  bundle: "套装",
  project: "项目",
  personal: "个人",
};

/**
 * 上下文地图（6.3、6.10 签名元素）：每一项按常驻占面积，总宽度是项目预算，剩余的部分用虚线画。
 * 颜色表示类型（黛蓝三档明度），斜纹表示来自全局层，圆点表示待处理，选中的块用正文色描边。
 * 和列表联动：悬停一块，列表那一行同时高亮；点一块就选中它。小于 9% 的块不写字。
 */
export function ContextMap({
  items,
  limit,
  registered,
  selectedId,
  hoverId,
  onHover,
  onPick,
  stagedDisabledIds,
  after,
}: {
  items: EffectiveItem[];
  limit: number;
  registered: boolean;
  selectedId: string | undefined;
  hoverId: string | null;
  onHover: (id: string | null) => void;
  onPick: (id: string) => void;
  /** 待停用释放预演集合。 */
  stagedDisabledIds?: ReadonlySet<string>;
  /** 排进计划还没同步的改动之后，常驻大约是多少（6.3「同步后约 N」）。 */
  after: number | null;
}) {
  const [mode, setMode] = useState<Mode>("kind");
  const counted = items.filter((i) => i.counted && (i.resident ?? 0) > 0);
  const order = [...counted].sort((a, b) =>
    mode === "layer"
      ? LAYER_ORDER.indexOf(a.scope) - LAYER_ORDER.indexOf(b.scope)
      : KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind),
  );
  const total = counted.reduce((n, i) => n + (i.resident ?? 0), 0);
  const cap = Math.max(limit, 1);
  const scale = Math.max(cap, total);
  const room = cap - total;
  const unestimated = items.filter((i) => i.kind === "mcp" && i.counted && i.resident === null);
  const axis = (mode === "layer" ? LAYER_ORDER : KIND_ORDER)
    .map((key) => {
      const v = order
        .filter((i) => (mode === "layer" ? i.scope === key : i.kind === key))
        .reduce((n, i) => n + (i.resident ?? 0), 0);
      const label = mode === "layer" ? LAYER_NAME[key as Scope] : KIND_NAME[key as AssetKind];
      return { key, label, v };
    })
    .filter((a) => a.v > 0);

  return (
    <section aria-label="常驻上下文" className={styles.map}>
      <div className={styles.headRow}>
        <div className={styles.headText}>
          <div className={styles.legendRow}>
            <span className={styles.cap}>常驻上下文 · 估算</span>
            <span className={styles.legend} title="斜纹：全局层带进来的">
              <span aria-hidden="true" className={styles.hatchSwatch} />
              全局
            </span>
            <span className={styles.legend} title="圆点：待处理（本地已改 · 可更新）">
              <span aria-hidden="true" className={styles.dot} data-tone="warn" />
              <span aria-hidden="true" className={styles.dot} data-tone="info" />
              待处理
            </span>
            <InfoTip label="地图说明">
              宽度按常驻估算，总宽度是{registered ? "项目预算" : "默认预算"}。斜纹是全局层带进来的，
              圆点是待处理（本地已改 · 可更新）。点一块看详情。
            </InfoTip>
          </div>
          <div className={styles.totals}>
            <span className={styles.total}>{formatK(total)}</span>
            <span className={styles.cap2}>
              / {registered ? "项目预算" : "默认预算"} {formatLimit(limit)} tokens
            </span>
            <span className={styles.left} data-over={room < 0}>
              {room >= 0 ? `剩余 ${formatK(room)}` : `超出 ${formatK(-room)}`}
            </span>
            {after !== null && <span className={styles.after}>· 同步后约 {formatK(after)}</span>}
            {unestimated.length > 0 && (
              <span className={styles.left}>· MCP {unestimated.length} 个未估算</span>
            )}
          </div>
        </div>
        <span className={styles.flex} />
        <Segmented
          label="地图分组"
          value={mode}
          onChange={setMode}
          options={[
            { value: "kind", label: "按类型" },
            { value: "layer", label: "按作用层" },
          ]}
        />
      </div>

      <div
        role="img"
        aria-label={`常驻上下文估算 ${formatK(total)}，占预算 ${formatLimit(limit)} 的 ${Math.round((total / cap) * 100)}%`}
        className={styles.blocks}
        onMouseLeave={() => onHover(null)}
      >
        {order.map((i) => {
          const w = ((i.resident ?? 0) / scale) * 100;
          const ring = i.state === "drifted" ? "warn" : i.state === "updatable" ? "info" : "none";
          const title = `${i.name} · ${formatK(i.resident ?? 0)} · ${LAYER_NAME[i.scope]} · ${i.label}`;
          return (
            <button
              key={i.id}
              type="button"
              className={styles.blk}
              data-kind={i.kind}
              data-glob={i.scope === "global"}
              data-staged={stagedDisabledIds?.has(i.id) || undefined}
              data-ring={ring}
              data-hl={hoverId === i.id}
              data-on={selectedId === i.id}
              style={{ width: `calc(${w.toFixed(2)}% - 3px)` }}
              title={title}
              aria-label={title}
              onClick={() => onPick(i.id)}
              onMouseEnter={() => onHover(i.id)}
              onFocus={() => onHover(i.id)}
            >
              <span className={styles.blkDot} aria-hidden="true" />
              {w >= 9 && (
                <>
                  <span className={styles.blkName}>{i.name}</span>
                  <span className={styles.blkCost}>{formatK(i.resident ?? 0)}</span>
                </>
              )}
            </button>
          );
        })}
        {room > 0 && (
          <div className={styles.room}>
            {room / scale >= 0.08 && <span>剩余 {formatK(room)}</span>}
          </div>
        )}
      </div>
      <div className={styles.axis} aria-hidden="true">
        {axis.map((a) => (
          <span
            key={a.key}
            data-key={a.key}
            style={{ width: `calc(${((a.v / scale) * 100).toFixed(2)}% - 3px)` }}
          >
            {a.label}
            <span className={styles.axisCost}>{formatK(a.v)}</span>
            {a.key === "global" && <span className={styles.axisNote}>每个项目都在缴</span>}
          </span>
        ))}
      </div>
    </section>
  );
}
