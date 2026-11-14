import { useState } from "react";
import { useNavigate } from "react-router";
import { Inspector, ReadOnlyAction, Section } from "@/components/Inspector";
import { StatusDot } from "@/components/StatusDot";
import { TONE_COLORS, type Tone } from "@/components/tone";
import type { Finding } from "@/ipc/bindings/Finding";
import type { FindingCategory } from "@/ipc/bindings/FindingCategory";
import type { LocationTone } from "@/ipc/bindings/LocationTone";
import { useCheckup } from "@/ipc/queries";
import page from "../page.module.css";
import styles from "./CheckupPage.module.css";

const CATEGORY: Record<FindingCategory, { label: string; tone: Tone }> = {
  duplicate: { label: "重复", tone: "warn" },
  "global-tax": { label: "全局税", tone: "warn" },
  inconsistent: { label: "不一致", tone: "info" },
  detached: { label: "脱节", tone: "warn" },
  secret: { label: "密钥", tone: "warn" },
};

const STATUS_LABEL: Record<Finding["status"], string> = {
  pending: "待处理",
  resolved: "已处理",
  "not-scanned": "尚未扫描",
};

function locationColor(tone: LocationTone | null): string {
  return tone ? TONE_COLORS[tone][0] : "var(--tx-2)";
}

/** 体检报告（6.2）：发现列表加检查器。首次启动先进这一页。 */
export function CheckupPage() {
  const report = useCheckup();
  const navigate = useNavigate();
  const [selected, setSelected] = useState<string | null>(null);
  const [inspectorOpen, setInspectorOpen] = useState(true);

  const data = report.data;
  const findings = data?.findings ?? [];
  const current = findings.find((f) => f.id === selected) ?? findings[0];
  const countable = findings.filter((f) => f.status !== "not-scanned");
  const done = countable.filter((f) => f.status === "resolved").length;

  return (
    <div className={page.page}>
      <header className={page.header}>
        <div className={page.titleGroup} style={{ alignItems: "center" }}>
          <h1 className={page.title}>体检报告</h1>
          {data && <StatusDot tone="ok" label="只读扫描完成，未改动任何文件" />}
          {report.isLoading && <StatusDot tone="muted" label="正在扫描…" />}
        </div>
        <button type="button" className={page.button} onClick={() => navigate(-1)}>
          稍后处理
        </button>
        <button type="button" className={page.buttonPrimary} onClick={() => navigate("/projects")}>
          进入项目
        </button>
      </header>

      {report.isError && <p className={page.error}>{report.error.message}</p>}

      {data && (
        <>
          <section aria-label="处理进度" className={page.band}>
            <div className={styles.summary}>
              <span className={styles.lead}>处理进度</span>
              <span className={styles.progress}>
                {done} / {countable.length}
              </span>
              <span className={styles.scanned}>
                扫描了 {data.toolsScanned} 个工具、{data.projectDirsScanned} 个项目目录，发现{" "}
                {data.assetsFound} 项资产
              </span>
              <span className={page.subtitle} style={{ flex: 1 }} />
              <span className={styles.stat}>重复副本 {data.duplicateCopies}</span>
              <span className={styles.stat}>来源未知 {data.unknownSource}</span>
            </div>
            <div
              role="img"
              aria-label={`已处理 ${done} / ${countable.length} 项发现`}
              className={styles.segments}
            >
              {countable.map((f) => (
                <span key={f.id} data-done={f.status === "resolved"} />
              ))}
            </div>
          </section>

          <div className={page.split}>
            <section className={page.list}>
              <div className={page.colHead}>
                <span className={styles.colTag}>类别</span>
                <span className={styles.colText}>发现</span>
                <span className={styles.colState}>状态</span>
              </div>
              {findings.length === 0 && (
                <p className={page.note}>
                  没有发现。各工具的全局层、{data.projectDirsScanned} 个项目目录都看过了。
                </p>
              )}
              {findings.map((f) => {
                const cat = CATEGORY[f.category];
                const resolved = f.status === "resolved";
                const tone: Tone = resolved
                  ? "ok"
                  : f.status === "not-scanned"
                    ? "muted"
                    : cat.tone;
                return (
                  <button
                    key={f.id}
                    type="button"
                    className={page.row}
                    aria-pressed={f.id === current?.id}
                    onClick={() => {
                      setSelected(f.id);
                      setInspectorOpen(true);
                    }}
                  >
                    <span className={styles.colTag} style={{ color: TONE_COLORS[tone][0] }}>
                      <span
                        aria-hidden="true"
                        className={styles.dot}
                        style={{ background: TONE_COLORS[tone][1] }}
                      />
                      {cat.label}
                    </span>
                    <span className={`${styles.colText} ${resolved ? page.struck : ""}`}>
                      {f.title}
                    </span>
                    <span
                      className={styles.colState}
                      style={resolved ? { color: "var(--ok)" } : undefined}
                    >
                      {STATUS_LABEL[f.status]}
                    </span>
                  </button>
                );
              })}
              <p className={styles.foot}>
                所有动作先预览、可撤销。处理记录留在「活动」里。「用到」依据本机近 30 天的{" "}
                {data.sessionsRead} 份会话记录，不上传。
              </p>
            </section>
            {inspectorOpen && current && (
              <Inspector
                kind="体检发现"
                name={current.title}
                status={{
                  tone: CATEGORY[current.category].tone,
                  label: CATEGORY[current.category].label,
                }}
                intro={current.intro}
                onClose={() => setInspectorOpen(false)}
              >
                <Section title="涉及的位置">
                  <div className={page.box}>
                    {current.locations.map((l) => (
                      <div key={`${l.path}:${l.detail}`} className={styles.location}>
                        <span className={styles.locPath}>{l.path}</span>
                        <span className={styles.locDetail}>{l.detail}</span>
                        <span
                          style={{ color: locationColor(l.tone), fontWeight: l.tone ? 500 : 400 }}
                        >
                          {l.note}
                        </span>
                      </div>
                    ))}
                  </div>
                  <span className={styles.locFoot}>{current.foot}</span>
                </Section>
                <ReadOnlyAction
                  label={current.action}
                  note="M0 只读，应用前会列出变更计划，在 M1 开放"
                />
              </Inspector>
            )}
          </div>
        </>
      )}
    </div>
  );
}
