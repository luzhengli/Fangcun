use std::sync::Arc;

use fangcun_core::Kernel;
use fangcun_core::error::ErrorPayload;

/// 应用级状态：全应用共用一个内核。
pub struct AppState {
    kernel: Arc<Kernel>,
}

impl AppState {
    pub fn new(kernel: Kernel) -> Self {
        Self {
            kernel: Arc::new(kernel),
        }
    }

    /// 在阻塞线程里跑一个内核用例。
    ///
    /// 内核是同步的，会做磁盘 IO。直接在命令里调会卡住界面，所以统一从这里走。
    pub async fn run<T, F>(&self, use_case: F) -> Result<T, ErrorPayload>
    where
        T: Send + 'static,
        F: FnOnce(&Kernel) -> fangcun_core::Result<T> + Send + 'static,
    {
        let kernel = Arc::clone(&self.kernel);
        tauri::async_runtime::spawn_blocking(move || use_case(&kernel))
            .await
            .map_err(|join_error| ErrorPayload::internal(join_error.to_string()))?
            .map_err(|error| ErrorPayload::from(&error))
    }
}
