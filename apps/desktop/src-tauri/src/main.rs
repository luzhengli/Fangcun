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
            AppState::watch(app.handle());
            AppState::check_upstream_in_background(app.handle());
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
            commands::library_overview,
            commands::activity,
            commands::plan_sync,
            commands::suggest_mode,
            commands::plan_init,
            commands::plan_adopt,
            commands::plan_rollback,
            commands::plan_rollback_record,
            commands::apply_plan,
            commands::pick_directory,
            commands::reveal_settings,
            commands::settings,
            commands::save_settings,
            commands::history,
            commands::manifest_view,
            commands::plan_finding,
            commands::plan_drift,
            commands::check_upstream,
            commands::plan_remove_global,
            commands::search_assets,
            commands::plan_add,
        ])
        .run(tauri::generate_context!())
        .expect("启动方寸失败");
}

const SETTINGS_MENU: &str = "settings";

/// 界面、Dock、菜单栏里的名字（9.3「包标识与显示名」）。`.app` 文件名和命令行仍是 ASCII 的 Fangcun、fangcun。
const DISPLAY_NAME: &str = "方寸";

/// 应用菜单。照 Tauri 的默认菜单（`Menu::default`）搭，但条目用中文、名字用显示名「方寸」，
/// 「关于」下面加「设置…」，快捷键 ⌘,（macOS 标准位置，6.1）。
fn add_settings_menu(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{
        AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu,
        WINDOW_SUBMENU_ID,
    };
    let h = app.handle();
    let about = AboutMetadata {
        name: Some(DISPLAY_NAME.to_owned()),
        version: Some(app.package_info().version.to_string()),
        ..Default::default()
    };
    let sep = || PredefinedMenuItem::separator(h);
    let app_menu = Submenu::with_items(
        h,
        DISPLAY_NAME,
        true,
        &[
            &PredefinedMenuItem::about(h, Some(&format!("关于{DISPLAY_NAME}")), Some(about))?,
            &sep()?,
            &MenuItem::with_id(h, SETTINGS_MENU, "设置…", true, Some("CmdOrCtrl+,"))?,
            &sep()?,
            &PredefinedMenuItem::services(h, Some("服务"))?,
            &sep()?,
            &PredefinedMenuItem::hide(h, Some(&format!("隐藏{DISPLAY_NAME}")))?,
            &PredefinedMenuItem::hide_others(h, Some("隐藏其他"))?,
            &PredefinedMenuItem::show_all(h, Some("全部显示"))?,
            &sep()?,
            &PredefinedMenuItem::quit(h, Some(&format!("退出{DISPLAY_NAME}")))?,
        ],
    )?;
    let file = Submenu::with_items(
        h,
        "文件",
        true,
        &[&PredefinedMenuItem::close_window(h, Some("关闭窗口"))?],
    )?;
    let edit = Submenu::with_items(
        h,
        "编辑",
        true,
        &[
            &PredefinedMenuItem::undo(h, Some("撤销"))?,
            &PredefinedMenuItem::redo(h, Some("重做"))?,
            &sep()?,
            &PredefinedMenuItem::cut(h, Some("剪切"))?,
            &PredefinedMenuItem::copy(h, Some("拷贝"))?,
            &PredefinedMenuItem::paste(h, Some("粘贴"))?,
            &PredefinedMenuItem::select_all(h, Some("全选"))?,
        ],
    )?;
    let view = Submenu::with_items(
        h,
        "显示",
        true,
        &[&PredefinedMenuItem::fullscreen(h, Some("进入全屏幕"))?],
    )?;
    let window = Submenu::with_id_and_items(
        h,
        WINDOW_SUBMENU_ID,
        "窗口",
        true,
        &[
            &PredefinedMenuItem::minimize(h, Some("最小化"))?,
            &PredefinedMenuItem::maximize(h, Some("缩放"))?,
        ],
    )?;
    let help = Submenu::with_id_and_items(h, HELP_SUBMENU_ID, "帮助", true, &[])?;
    let menu = Menu::with_items(h, &[&app_menu, &file, &edit, &view, &window, &help])?;
    app.set_menu(menu)?;
    Ok(())
}
