import { useEffect, useState } from "react";
import { Icon } from "@/components/Icon";
import type { SettingsForm } from "@/ipc/bindings/SettingsForm";
import { useSaveSettings, useSettings } from "@/ipc/queries";
import { PageToolbar } from "@/shell/PageToolbar";
import styles from "./SettingsPage.module.css";

/**
 * 设置（6.1、M1-24）：主窗口里显示，侧栏照常显示，右边是设置目录与内容。
 * 先只有「通用」一页，分「写入」「快照」两组。改了立刻存进 `~/.fangcun/settings.toml`。
 */
export function SettingsPage() {
  const settings = useSettings();
  const save = useSaveSettings();
  const form = settings.data;

  return (
    <div className={styles.page}>
      <PageToolbar title="设置" subtitle="⌘, · 改完立即生效" />
      <aside aria-label="设置目录" className={styles.directory}>
        <button type="button" aria-current="page" className={styles.directoryItem}>
          <span className={styles.directoryIcon}>
            <Icon name="settings" />
          </span>
          <span>通用</span>
        </button>
      </aside>
      <main className={styles.content}>
        <div className={styles.column}>
          <h2 className={styles.title}>通用</h2>
          {settings.isError && <p className={styles.error}>{settings.error.message}</p>}
          {save.isError && <p className={styles.error}>{save.error.message}</p>}
          {form && (
            <GeneralForm key={JSON.stringify(form)} form={form} onSave={(f) => save.mutate(f)} />
          )}
          <p className={styles.foot}>
            设置存在 <span className={styles.mono}>~/.fangcun/settings.toml</span>，可以手改。
          </p>
        </div>
      </main>
    </div>
  );
}

function GeneralForm({
  form,
  onSave,
}: {
  form: SettingsForm;
  onSave: (form: SettingsForm) => void;
}) {
  // 数字框先在本地改，失焦或回车再存；填错（空、0）时回到原值。
  const [days, setDays] = useState(String(form.snapshotDays));
  const [keep, setKeep] = useState(String(form.snapshotKeep));
  useEffect(() => {
    setDays(String(form.snapshotDays));
    setKeep(String(form.snapshotKeep));
  }, [form]);
  const commit = () => {
    const d = Number.parseInt(days, 10);
    const k = Number.parseInt(keep, 10);
    if (!(d > 0) || !(k > 0)) {
      setDays(String(form.snapshotDays));
      setKeep(String(form.snapshotKeep));
      return;
    }
    if (d !== form.snapshotDays || k !== form.snapshotKeep)
      onSave({ ...form, snapshotDays: d, snapshotKeep: k });
  };
  const onEnter = (e: React.KeyboardEvent) => e.key === "Enter" && commit();

  return (
    <>
      <h3 className={styles.group}>写入</h3>
      <div className={styles.card}>
        <div className={styles.row}>
          <div className={styles.rowText}>
            <span id="ro-label" className={styles.rowTitle}>
              只读模式
            </span>
            <span className={styles.rowDesc}>
              打开后拒绝一切写入，只记体检。命令行同款：
              <span className={styles.mono}>fangcun --read-only &lt;命令&gt;</span>
            </span>
          </div>
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
      </div>

      <h3 className={`${styles.group} ${styles.groupGap}`}>快照</h3>
      <div className={styles.card}>
        <div className={styles.row}>
          <div className={styles.rowText}>
            <label htmlFor="days" className={styles.rowTitle}>
              保留天数
            </label>
            <span className={styles.rowDesc}>
              快照和它的备份保留多久。过期后记录仍在，回滚按钮置灰。
            </span>
          </div>
          <span className={styles.field}>
            <input
              id="days"
              type="number"
              min={1}
              max={365}
              value={days}
              onChange={(e) => setDays(e.target.value)}
              onBlur={commit}
              onKeyDown={onEnter}
            />
            <span className={styles.unit}>天</span>
          </span>
        </div>
        <div className={styles.row}>
          <div className={styles.rowText}>
            <label htmlFor="keep" className={styles.rowTitle}>
              每个项目至少保留
            </label>
            <span className={styles.rowDesc}>
              超过 {form.snapshotDays} 天也不清理，每个项目留下最近的这么多个。
            </span>
          </div>
          <span className={styles.field}>
            <input
              id="keep"
              type="number"
              min={1}
              max={500}
              value={keep}
              onChange={(e) => setKeep(e.target.value)}
              onBlur={commit}
              onKeyDown={onEnter}
            />
            <span className={styles.unit}>个</span>
          </span>
        </div>
      </div>
    </>
  );
}
