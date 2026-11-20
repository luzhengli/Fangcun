//! 方寸桌面应用的外壳。
//!
//! 这一层只做三件事：打开内核、把内核的用例暴露成 Tauri 命令、把内核事件转发给界面。
//! 业务逻辑一律放在 `fangcun-core`。

// 发布版在 Windows 上不弹控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use fangcun_core::Kernel;
use tauri::Manager;

use crate::state::AppState;

fn main() {
    tauri::Builder::default()
        // 只给外壳用（`commands::pick_directory`），capabilities 里不开它的权限，界面调不到。
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 默认可写；设置里开了只读模式时只读（7.5）。
            let kernel = Kernel::open_configured(false)?;
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
            commands::plan_sync,
            commands::suggest_mode,
            commands::plan_init,
            commands::plan_adopt,
            commands::plan_rollback,
            commands::plan_rollback_record,
            commands::apply_plan,
            commands::pick_directory,
        ])
        .run(tauri::generate_context!())
        .expect("启动方寸失败");
}
