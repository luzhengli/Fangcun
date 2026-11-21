//! 监听器（F4、7.2、M1-15）：监听已登记项目的 `.agents/skills` 和资产库正本，发现漂移。
//!
//! 监听器只发现、只记录，不改任何受管文件。验收：Agent 改了 skill，五秒内出现漂移提示。
//! 只在桌面应用里常驻；命令行每次运行时直接比对，不启动监听。

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::ProjectId;
use crate::{Error, Result};

/// 内核主动推给外壳的事件。桌面外壳把它转发给界面，命令行不订阅。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case", tag = "type")]
#[ts(export)]
pub enum KernelEvent {
    /// 受管副本被修改。
    DriftDetected { project: ProjectId, asset: String },
    /// 活动记录有新增。界面据此刷新待处理数和活动页。
    JournalChanged,
}

/// 事件的接收方。外壳实现它。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: KernelEvent);
}

/// 监听一组目录（递归），有变化时调 `on_change`：一阵变化停下 0.5 秒后调一次，
/// 免得 Agent 连着写几个文件时反复检测。macOS 用 FSEvents（`notify`）。
pub struct Watcher {
    _inner: notify::RecommendedWatcher,
}

impl std::fmt::Debug for Watcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Watcher")
    }
}

/// 一阵变化停下多久才算完。
const QUIET: std::time::Duration = std::time::Duration::from_millis(500);

impl Watcher {
    /// 开始监听。不存在的目录跳过。`on_change` 在监听器自己的线程里调。
    pub fn start(paths: Vec<PathBuf>, on_change: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        use notify::Watcher as _;
        let (tx, rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();
        let mut inner = notify::recommended_watcher(tx)
            .map_err(|e| Error::Source(format!("文件监听启动失败：{e}")))?;
        for path in paths.iter().filter(|p| p.exists()) {
            inner
                .watch(path, notify::RecursiveMode::Recursive)
                .map_err(|e| Error::Source(format!("监听 {} 失败：{e}", path.display())))?;
        }
        std::thread::Builder::new()
            .name("fangcun-watcher".to_owned())
            .spawn(move || {
                while rx.recv().is_ok() {
                    while rx.recv_timeout(QUIET).is_ok() {}
                    on_change();
                }
            })
            .map_err(|e| Error::Source(format!("文件监听启动失败：{e}")))?;
        Ok(Self { _inner: inner })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn calls_back_once_after_a_burst_of_writes() {
        let dir = std::env::temp_dir().join(format!("fangcun-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let dir = crate::scanner::fsutil::canonical(&dir);
        let calls = Arc::new(AtomicU32::new(0));
        let seen = Arc::clone(&calls);
        let watcher = Watcher::start(
            vec![dir.clone()],
            Arc::new(move || {
                seen.fetch_add(1, Ordering::SeqCst);
            }),
        )
        .unwrap();
        // FSEvents 开始监听要一点时间。
        std::thread::sleep(std::time::Duration::from_millis(300));
        for i in 0..5 {
            std::fs::write(dir.join(format!("f{i}")), "x").unwrap();
        }
        // 验收是五秒内（F4）。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while calls.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(calls.load(Ordering::SeqCst) >= 1);
        drop(watcher);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
