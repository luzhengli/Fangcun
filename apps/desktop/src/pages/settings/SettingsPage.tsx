import { useEffect, useState } from "react";
import c from "@/components/controls.module.css";
import { Icon } from "@/components/Icon";
import { InfoTip } from "@/components/InfoTip";
import type { SettingsForm } from "@/ipc/bindings/SettingsForm";
import { kernel } from "@/ipc/commands";
import { useSaveSettings, useSettings } from "@/ipc/queries";
import { PageToolbar } from "@/shell/PageToolbar";
import styles from "./SettingsPage.module.css";

/**
 * 设置（6.1，方寸 2）：主窗口里显示，侧栏照常显示，右边是设置目录与内容。
 * 先只有「通用」一页，分「写入」「快照」「隐私」三组。改了立刻存进 `~/.fangcun/settings.toml`。
 */
export function SettingsPage() {
  const settings = useSettings();
  const save = useSaveSettings();
  const form = settings.data;
  const [revealError, setRevealError] = useState<string | null>(null);

  return (
    <div className={styles.page}>
      <PageToolbar title="设置" subtitle="⌘, · 改完立即生效" />
      <nav aria-label="设置目录" className={styles.directory}>
        <a href="#/settings" aria-current="page" className={styles.nav}>
          <Icon name="general" size={16} strokeWidth={1.4} />
          <span>通用</span>
        </a>
      </nav>
      <main className={styles.content}>
        <div className={styles.column}>
          <h2 className={styles.title}>通用</h2>
          {settings.isError && <p className={styles.error}>{settings.error.message}</p>}
          {save.isError && <p className={styles.error}>{save.error.message}</p>}
          {form && <GeneralForm form={form} onSave={(f) => save.mutate(f)} />}
          <div className={styles.foot}>
            <span className={styles.flex} title="可以手改，改完这里立即生效">
              <span className={styles.mono}>~/.fangcun/settings.toml</span> · 可手改
            </span>
            <button
              type="button"
              className={`${c.b} ${c.small}`}
              onClick={() => {
                setRevealError(null);
                kernel
                  .revealSettings()
                  .catch((e: unknown) =>
                    setRevealError(e instanceof Error ? e.message : String(e)),
                  );
              }}
            >
              在访达显示
            </button>
          </div>
          {revealError && <p className={styles.error}>{revealError}</p>}
        </div>
      </main>
    </div>
  );
}

/** 「− N +」步进器（6.1「快照」）。 */
function Stepper({
  label,
  value,
  min,
  max,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (n: number) => void;
}) {
  return (
    <fieldset className={styles.stepper} aria-label={label}>
      <button
        type="button"
        className={styles.step}
        aria-label={`${label}减少`}
        disabled={value <= min}
        onClick={() => onChange(Math.max(min, value - 1))}
      >
        −
      </button>
      <output className={styles.stepValue} aria-live="polite">
        {value}
      </output>
      <button
        type="button"
        className={styles.step}
        aria-label={`${label}增加`}
        disabled={value >= max}
        onClick={() => onChange(Math.min(max, value + 1))}
      >
        +
      </button>
    </fieldset>
  );
}

function GeneralForm({
  form,
  onSave,
}: {
  form: SettingsForm;
  onSave: (form: SettingsForm) => void;
}) {
  // 连点「+」时先在本地变，停下 400ms 再存一次，免得每点一下写一次文件。
  const [days, setDays] = useState(form.snapshotDays);
  const [keep, setKeep] = useState(form.snapshotKeep);
  useEffect(() => {
    setDays(form.snapshotDays);
    setKeep(form.snapshotKeep);
  }, [form.snapshotDays, form.snapshotKeep]);
  useEffect(() => {
    if (days === form.snapshotDays && keep === form.snapshotKeep) return;
    const t = window.setTimeout(
      () => onSave({ ...form, snapshotDays: days, snapshotKeep: keep }),
      400,
    );
    return () => window.clearTimeout(t);
  }, [days, keep, form, onSave]);

  return (
    <>
      <section className={styles.group}>
        <h3 className={styles.cap}>写入</h3>
        <div className={styles.card}>
          <div className={styles.row}>
            <span className={styles.rowText}>
              <span id="ro-label" className={styles.rowTitle}>
                只读模式
              </span>
              <span className={styles.rowDesc} title="命令行同款：fangcun --read-only <命令>">
                拒绝一切写入，只体检和查看
              </span>
            </span>
            <button
              type="button"
              role="switch"
              aria-checked={form.readOnly}
              aria-labelledby="ro-label"
              className={styles.switch}
              onClick={() => onSave({ ...form, readOnly: !form.readOnly })}
            >
              <span aria-hidden="true" />
            </button>
          </div>
          {form.readOnly && (
            <div className={styles.roNote}>
              <span className={styles.dot} />
              只读中 ·「同步」「应用」变灰
            </div>
          )}
        </div>
      </section>

      <section className={styles.group}>
        <h3 className={styles.cap}>快照</h3>
        <div className={styles.card}>
          <div className={styles.row}>
            <span className={styles.rowText}>
              <span className={styles.rowTitle}>保留天数</span>
              <span className={styles.rowDesc}>过期后记录还在，不能再回滚</span>
            </span>
            <Stepper label="保留天数" value={days} min={1} max={365} onChange={setDays} />
            <span className={styles.unit}>天</span>
          </div>
          <div className={styles.row}>
            <span className={styles.rowText}>
              <span className={styles.rowTitle}>每个项目至少保留</span>
              <span className={styles.rowDesc}>超过天数也不清理</span>
            </span>
            <Stepper label="每个项目至少保留" value={keep} min={1} max={200} onChange={setKeep} />
            <span className={styles.unit}>个</span>
          </div>
        </div>
        <span className={styles.note}>
          {days} 天后清理 · 每个项目至少留 {keep} 个
        </span>
      </section>

      <section className={styles.group}>
        <h3 className={styles.cap}>隐私</h3>
        <div className={styles.card}>
          <div className={styles.row}>
            <Icon name="shieldCheck" size={18} strokeWidth={1.4} className={styles.shield} />
            <span className={styles.rowText}>
              <span className={styles.rowTitle}>本地优先 · 零遥测</span>
              <span className={styles.rowDesc}>
                不需要账号，不上报使用数据
                <InfoTip label="统计说明">
                  「谁在用」的统计来自本机的会话记录，不离开这台电脑。联网只为检查你声明的上游来源。
                </InfoTip>
              </span>
            </span>
          </div>
        </div>
      </section>
    </>
  );
}
