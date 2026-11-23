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
 * 上下文地图（6.3、6.10 签名元素）：拆清「预算水位（4px 细轨）」与「已估内部构成（32px 满宽条）」。
 * 细轨按项目预算缩放；构成条按已估常驻 100% 铺开，低占比项目也能看清大头名称；
 * 未估算 MCP 在构成条末尾以虚点块提示，点击定位到列表 MCP 组。
 */
export function ContextMap({
  items,
  limit,
  registered,
  selectedId,
  hoverId,
  onHover,
  onPick,
  onPickUnestimated,
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
  /** 点击「MCP N 项未估」虚点块时定位至列表 MCP 组。 */
  onPickUnestimated?: () => void;
  /** 待停用释放预演集合。 */
  stagedDisabledIds?: ReadonlySet<string>;
  /** 排进计划还没同步的改动之后，常驻大约是多少（6.3「同步后约 N」）。 */
  after: number | null;
}) {
  const [mode, setMode] = useState<Mode>("kind");
  const counted = items.filter((i) => i.counted && (i.resident ?? 0) > 0);
  const order = [...counted].sort((a, b) => {
    const groupDiff =
      mode === "layer"
        ? LAYER_ORDER.indexOf(a.scope) - LAYER_ORDER.indexOf(b.scope)
        : KIND_ORDER.indexOf(a.kind) - KIND_ORDER.indexOf(b.kind);
    if (groupDiff !== 0) return groupDiff;
    const residentDiff = (b.resident ?? 0) - (a.resident ?? 0);
    if (residentDiff !== 0) return residentDiff;
    return a.name.localeCompare(b.name);
  });
  const total = counted.reduce((n, i) => n + (i.resident ?? 0), 0);
  const cap = Math.max(limit, 1);
  const scale = Math.max(cap, total);
  const room = cap - total;
  const pct = Math.round((total / cap) * 100);
  const unestimated = items.filter((i) => i.kind === "mcp" && i.counted && i.resident === null);
  const hasUnestimated = unestimated.length > 0;
  const hasGlobal = order.some((i) => i.scope === "global");
  const hasPending = order.some((i) => i.state === "drifted" || i.state === "updatable");
  const selectedUnestimated = unestimated.some((i) => i.id === selectedId);
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
    <section aria-label="常驻上下文" className={styles.map} data-no-card="true">
      <div className={styles.headRow}>
        <div className={styles.metrics}>
          <span className={styles.cap}>常驻 · 估算</span>
          <span className={styles.total}>
            {hasUnestimated && <span className={styles.estPrefix}>已估 </span>}
            {formatK(total)}
          </span>
          <span className={styles.cap2}>
            / {registered ? "项目预算" : "默认预算"} {formatLimit(limit)} ({pct}%)
          </span>
          {order.length === 0 && !hasUnestimated ? (
            <span className={styles.left}>· 没有计入常驻的项</span>
          ) : (
            <>
              <span className={styles.sep} aria-hidden="true">
                ·
              </span>
              <span className={styles.left} data-over={room < 0}>
                {room >= 0
                  ? `剩余 ${hasUnestimated ? "≤" : ""}${formatK(room)}`
                  : `超出 ${formatK(-room)}`}
              </span>
            </>
          )}
          {after !== null && <span className={styles.after}>· 同步后约 {formatK(after)}</span>}
          <InfoTip label="地图说明">
            上方细轨按{registered ? "项目预算" : "默认预算"}
            显示总体水位；下方色条将已估算常驻铺满整行，方便看清大头构成。斜纹是全局层带进来的，圆点是待处理（本地已改
            · 可更新），虚点块为未估算的 MCP。点一块查看详情。
          </InfoTip>
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
        aria-label={`常驻上下文估算 ${hasUnestimated ? "已估 " : ""}${formatK(total)}，占预算 ${formatLimit(limit)} 的 ${pct}%${
          hasUnestimated ? `，另有 ${unestimated.length} 个 MCP 未估算` : ""
        }`}
        className={styles.rail}
      >
        {axis.map((a) => (
          <span
            key={a.key}
            className={styles.railSeg}
            data-key={a.key}
            style={{ width: `${((a.v / scale) * 100).toFixed(2)}%` }}
          />
        ))}
        {hasUnestimated && room > 0 && (
          <span
            className={styles.railUnest}
            style={{ width: `min(24px, ${((room / scale) * 100).toFixed(2)}%)` }}
            aria-hidden="true"
          />
        )}
        {room < 0 && (
          <span
            className={styles.railCap}
            style={{ left: `${((cap / scale) * 100).toFixed(2)}%` }}
            aria-hidden="true"
          />
        )}
      </div>

      {(order.length > 0 || hasUnestimated) && (
        <fieldset
          aria-label="已估常驻构成"
          className={styles.blocks}
          onMouseLeave={() => onHover(null)}
        >
          {order.length > 0 && (
            <div className={styles.estimated}>
              {order.map((i) => {
                const resident = i.resident ?? 0;
                const share = total > 0 ? (resident / total) * 100 : 0;
                const showName = share >= 10;
                const showCost = share >= 16;
                const ring =
                  i.state === "drifted" ? "warn" : i.state === "updatable" ? "info" : "none";
                const title = `${i.name} · ${formatK(resident)}（占已估 ${Math.round(share)}%）· ${LAYER_NAME[i.scope]} · ${i.label}`;
                return (
                  <button
                    key={i.id}
                    type="button"
                    className={styles.blk}
                    data-kind={i.kind}
                    data-glob={i.scope === "global"}
                    data-staged={stagedDisabledIds?.has(i.id) || undefined}
                    data-ring={ring}
                    data-labeled={showName || undefined}
                    data-hl={hoverId === i.id}
                    data-on={selectedId === i.id}
                    style={{ flexGrow: Math.max(resident, 1) }}
                    title={title}
                    aria-label={title}
                    onClick={() => onPick(i.id)}
                    onMouseEnter={() => onHover(i.id)}
                    onFocus={() => onHover(i.id)}
                  >
                    <span className={styles.blkDot} aria-hidden="true" />
                    {showName && <span className={styles.blkName}>{i.name}</span>}
                    {showCost && <span className={styles.blkCost}>{formatK(resident)}</span>}
                  </button>
                );
              })}
            </div>
          )}
          {hasUnestimated && (
            <button
              type="button"
              className={styles.unestBlk}
              data-solo={order.length === 0 || undefined}
              data-on={selectedUnestimated || undefined}
              onClick={onPickUnestimated}
              title={`${unestimated.length} 个 MCP 服务未估算（需启动服务读取工具定义），点击定位到 MCP 组`}
              aria-label={`MCP ${unestimated.length} 项未估，点击定位到 MCP 组`}
            >
              <span>
                MCP {unestimated.length} 项未估{order.length === 0 ? " · 点击定位" : ""}
              </span>
            </button>
          )}
        </fieldset>
      )}

      {(axis.length > 0 || hasUnestimated) && (
        <div className={styles.foot}>
          <div className={styles.summary}>
            {axis.map((a, idx) => (
              <span key={a.key} className={styles.summaryItem}>
                {idx > 0 && (
                  <span className={styles.sep} aria-hidden="true">
                    ·
                  </span>
                )}
                <span aria-hidden="true" className={styles.swatch} data-key={a.key} />
                <span>{a.label}</span>
                <span className={styles.axisCost}>{formatK(a.v)}</span>
                {a.key === "global" && <span className={styles.axisNote}>（每个项目都在缴）</span>}
              </span>
            ))}
            {hasUnestimated && (
              <span className={styles.summaryItem}>
                {axis.length > 0 && (
                  <span className={styles.sep} aria-hidden="true">
                    ·
                  </span>
                )}
                <span aria-hidden="true" className={styles.unestSwatch} />
                <span className={styles.axisNote}>MCP {unestimated.length} 个未估算</span>
              </span>
            )}
          </div>
          {(hasGlobal || hasPending) && (
            <div className={styles.legends}>
              {hasGlobal && (
                <span className={styles.legend} title="斜纹：全局层带进来的">
                  <span aria-hidden="true" className={styles.hatchSwatch} />
                  全局
                </span>
              )}
              {hasPending && (
                <span className={styles.legend} title="圆点：待处理（本地已改 · 可更新）">
                  <span aria-hidden="true" className={styles.dot} data-tone="warn" />
                  <span aria-hidden="true" className={styles.dot} data-tone="info" />
                  待处理
                </span>
              )}
            </div>
          )}
        </div>
      )}
    </section>
  );
}
