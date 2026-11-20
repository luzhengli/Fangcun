//! 方寸桌面应用的外壳。
//!
//! 这一层只做三件事：打开内核、把内核的用例暴露成 Tauri 命令、把内核事件转发给界面。
//! 业务逻辑一律放在 `fangcun-core`。

// 发布版在 Windows 上不弹控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use fangcun_core::Kernel;
use tauri::{Emitter, Manager};

use crate::state::AppState;

fn main() {
    tauri::Builder::default()
        // 只给外壳用（`commands::pick_directory`），capabilities 里不开它的权限，界面调不到。
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 默认可写；设置里开了只读模式时只读（7.5）。
            let kernel = Kernel::open_configured(false)?;
            app.manage(AppState::new(kernel));
            add_settings_menu(app)?;
            Ok(())
        })
        // 应用菜单「设置…」（⌘,）：让界面切到设置页（6.1）。
        .on_menu_event(|app, event| {
            if event.id() == SETTINGS_MENU {
                let _ = app.emit("open-settings", ());
            }
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
            commands::settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("启动方寸失败");
}

const SETTINGS_MENU: &str = "settings";

/// 在系统默认菜单的应用菜单里，「关于」下面加「设置…」，快捷键 ⌘,（macOS 标准位置）。
fn add_settings_menu(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, MenuItemKind, PredefinedMenuItem};
    let menu = Menu::default(app.handle())?;
    if let Some(MenuItemKind::Submenu(app_menu)) = menu.items()?.into_iter().next() {
        let settings = MenuItem::with_id(app, SETTINGS_MENU, "设置…", true, Some("CmdOrCtrl+,"))?;
        app_menu.insert(&settings, 2)?;
        app_menu.insert(&PredefinedMenuItem::separator(app)?, 3)?;
    }
    app.set_menu(menu)?;
    Ok(())
}
