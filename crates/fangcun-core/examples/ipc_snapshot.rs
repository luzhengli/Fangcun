//! 开发用：用真实内核跑一遍界面要的全部只读命令，把结果写成一份 JSON。
//!
//! `pnpm screens --snapshot <文件>` 读它，在浏览器里用本机的真实数据渲染界面并截图，
//! 不用开桌面窗口。只读：内核以 `Access::ReadOnly` 打开，只写参数给的那一个文件，
//! 外加和桌面应用一样往 `~/.fangcun/journal` 记一条体检。
//!
//! 用法：cargo run -p fangcun-core --example ipc_snapshot -- target/ipc-snapshot.json

use std::collections::BTreeMap;

use fangcun_core::journal::RecordFilter;
use fangcun_core::model::ToolId;
use fangcun_core::{Access, Kernel};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::env::args().nth(1).ok_or("要给出输出文件")?;
    let kernel = Kernel::open(Access::ReadOnly)?;
    let tools = kernel.tools();
    let full: Vec<ToolId> = tools
        .iter()
        .filter(|t| t.support == fangcun_core::adapter::SupportLevel::Full)
        .map(|t| t.id.clone())
        .collect();
    let checkup = kernel.doctor()?;
    let projects = kernel.projects()?;
    let mut effective: BTreeMap<String, Value> = BTreeMap::new();
    for project in &projects {
        for tool in &full {
            let context = kernel.effective_context(std::path::Path::new(&project.id.0), tool)?;
            effective.insert(
                format!("{}|{}", project.id.0, tool.0),
                serde_json::to_value(context)?,
            );
        }
    }
    let mut global: BTreeMap<String, Value> = BTreeMap::new();
    for tool in &full {
        global.insert(
            tool.0.clone(),
            serde_json::to_value(kernel.global_overview(tool)?)?,
        );
    }
    let snapshot = json!({
        "tools": tools,
        "overview": kernel.overview()?,
        "projects": projects,
        "checkup": checkup,
        "effective": effective,
        "global": global,
        "activity": kernel.log(&RecordFilter::default())?,
    });
    std::fs::write(&out, serde_json::to_string(&snapshot)?)?;
    eprintln!("写好了 {out}");
    Ok(())
}
