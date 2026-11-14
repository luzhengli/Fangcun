//! 方寸桌面应用的外壳。
//!
//! 这一层只做三件事：打开内核、把内核的用例暴露成 Tauri 命令、把内核事件转发给界面。
//! 业务逻辑一律放在 `fangcun-core`。

// 发布版在 Windows 上不弹控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use fangcun_core::{Access, Kernel};
use tauri::Manager;

use crate::state::AppState;

/// M0 只读。M1 起改为 `Access::ReadWrite`。
const ACCESS: Access = Access::ReadOnly;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let kernel = Kernel::open(ACCESS)?;
            app.manage(AppState::new(kernel));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::tools,
            commands::overview,
            commands::projects,
            commands::checkup,
            commands::rescan,
            commands::effective_context,
            commands::why,
            commands::global_overview,
            commands::activity,
        ])
        .run(tauri::generate_context!())
        .expect("启动方寸失败");
}
