use std::sync::{Arc, RwLock};

use fangcun_core::Kernel;
use fangcun_core::error::ErrorPayload;

/// 应用级状态：全应用共用一个内核。设置页切换只读模式后换一个新内核（7.5）。
pub struct AppState {
    kernel: RwLock<Arc<Kernel>>,
}

impl AppState {
    pub fn new(kernel: Kernel) -> Self {
        Self {
            kernel: RwLock::new(Arc::new(kernel)),
        }
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
