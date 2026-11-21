use std::sync::{Arc, Mutex, RwLock};

use fangcun_core::Kernel;
use fangcun_core::error::ErrorPayload;
use fangcun_core::watcher::{KernelEvent, Watcher};
use tauri::{Emitter, Manager};

/// 应用级状态：全应用共用一个内核。设置页切换只读模式后换一个新内核（7.5）。
pub struct AppState {
    kernel: RwLock<Arc<Kernel>>,
    /// 文件监听（M1-15）。登记了项目、应用了计划后按新的目录重设。
    watcher: Mutex<Option<Watcher>>,
}

/// 内核事件发给界面的事件名。界面收到后让对应的查询失效。
pub const KERNEL_EVENT: &str = "kernel-event";

impl AppState {
    pub fn new(kernel: Kernel) -> Self {
        Self {
            kernel: RwLock::new(Arc::new(kernel)),
            watcher: Mutex::new(None),
        }
    }

    /// 按现在登记的项目重设文件监听：有变化就重新检测漂移，新的漂移推给界面（F4：五秒内出提示）。
    pub fn watch(app: &tauri::AppHandle) {
        let state = app.state::<AppState>();
        let paths = state.current().watch_paths();
        let handle = app.clone();
        let started = Watcher::start(
            paths,
            Arc::new(move || {
                let kernel = handle.state::<AppState>().current();
                if let Ok(events) = kernel.refresh_drift() {
                    for event in events {
                        let _ = handle.emit(KERNEL_EVENT, event);
                    }
                }
            }),
        );
        if let Ok(mut guard) = state.watcher.lock() {
            *guard = started.ok();
        }
    }

    /// 启动时在后台做一次上游检查（9.3：启动时一次 + 手动），查到新版本就让界面刷新。
    pub fn check_upstream_in_background(app: &tauri::AppHandle) {
        let handle = app.clone();
        std::thread::spawn(move || {
            let kernel = handle.state::<AppState>().current();
            if let Ok(report) = kernel.check_upstream()
                && !report.items.is_empty()
            {
                let _ = handle.emit(KERNEL_EVENT, KernelEvent::JournalChanged);
            }
        });
    }

    fn current(&self) -> Arc<Kernel> {
        match self.kernel.read() {
            Ok(guard) => Arc::clone(&guard),
            Err(poisoned) => Arc::clone(&poisoned.into_inner()),
        }
    }

    /// 按设置重新打开内核。旧内核里没应用的计划随之作废。
    pub fn reopen(&self) -> Result<(), ErrorPayload> {
        let kernel = Kernel::open_configured(false).map_err(|error| ErrorPayload::from(&error))?;
        match self.kernel.write() {
            Ok(mut guard) => *guard = Arc::new(kernel),
            Err(poisoned) => *poisoned.into_inner() = Arc::new(kernel),
        }
        Ok(())
    }

    /// 在阻塞线程里跑一个内核用例。
    ///
    /// 内核是同步的，会做磁盘 IO。直接在命令里调会卡住界面，所以统一从这里走。
    pub async fn run<T, F>(&self, use_case: F) -> Result<T, ErrorPayload>
    where
        T: Send + 'static,
        F: FnOnce(&Kernel) -> fangcun_core::Result<T> + Send + 'static,
    {
        let kernel = self.current();
        tauri::async_runtime::spawn_blocking(move || use_case(&kernel))
            .await
            .map_err(|join_error| ErrorPayload::internal(join_error.to_string()))?
            .map_err(|error| ErrorPayload::from(&error))
    }
}
