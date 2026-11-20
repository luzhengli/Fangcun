//! 项目页检查器里从台账和活动记录来的三样（6.3 检查器第 5、6 条，M1-17、M1-32、M1-33）：
//!
//! - 用了密钥的 MCP：方寸写在个人层、带明文的那一条显示「密钥在钥匙串」，项目层那条引用并进它的
//!   「在哪一层声明」；只写了环境变量引用的（Codex）显示「密钥取自环境变量」。
//! - 写入了哪些文件：台账里这一项的路径、软链指向和按键合并的键。
//! - 最近活动：这个项目的活动记录里提到这一项的，新的在前。

use std::collections::BTreeMap;
use std::path::Path;

use crate::adapter::Adapter;
use crate::journal::Record;
use crate::materializer::ledger::{KeyKind, Ledger, PathKind};
use crate::model::{AssetKind, Scope};
use crate::resolver::{EffectiveContext, EffectiveItem, ItemActivity};

/// 「最近活动」最多列几条。
const RECENT_LIMIT: usize = 5;

pub(super) fn attach(
    context: &mut EffectiveContext,
    adapter: &Adapter,
    ledger: &Ledger,
    records: &[Record],
    library: &Path,
) {
    attach_secrets(context, adapter, ledger);
    for item in &mut context.items {
        let names = names_of(item);
        item.files = written(item, &names, ledger, library);
        item.recent = recent(item, &names, records);
    }
}

/// 这一项在台账和记录里用的名字：合成一行的是全部成员，指令文件是文件名。
fn names_of(item: &EffectiveItem) -> Vec<String> {
    if item.members.is_empty() {
        vec![item.name.clone()]
    } else {
        item.members.clone()
    }
}

fn attach_secrets(context: &mut EffectiveContext, adapter: &Adapter, ledger: &Ledger) {
    let tool_name = adapter.tool.name.clone();
    let forward = adapter
        .mcp_fields
        .as_ref()
        .and_then(|f| f.env_forward.clone());
    let mut folded: Vec<String> = Vec::new();
    for item in context
        .items
        .iter_mut()
        .filter(|i| i.kind == AssetKind::Mcp)
    {
        let key = ledger.keys.iter().find(|k| {
            k.asset == item.name
                && match item.scope {
                    Scope::Personal => {
                        k.kind == KeyKind::McpPersonal && k.tool.as_ref() == Some(&adapter.tool.id)
                    }
                    Scope::Project => k.kind == KeyKind::Mcp,
                    _ => false,
                }
        });
        let Some(key) = key else {
            continue;
        };
        if item.scope == Scope::Personal {
            if key.secrets.is_empty() {
                item.reason.push_str(
                    "\n它来自个人覆盖 fangcun.local.toml，方寸写在本机个人层，不提交，同事看不到。",
                );
                continue;
            }
            item.label = "密钥在钥匙串".to_owned();
            item.state = None;
            item.secrets = key
                .secrets
                .iter()
                .map(|(var, name)| format!("{var} → 钥匙串 {name}"))
                .collect();
            item.reason = format!(
                "项目清单里声明了这个 MCP 服务，所以它对本项目生效。\n令牌存在钥匙串里，提交的配置只写环境变量引用；带明文的那一条方寸写在本机个人层，{tool_name} 同名时个人层优先。"
            );
            folded.push(item.id.clone());
        } else if !key.secrets.is_empty() && item.overridden_by.is_none() {
            let vars: Vec<&String> = key.secrets.keys().collect();
            if forward.is_some() {
                item.label = "密钥取自环境变量".to_owned();
                item.secrets = key
                    .secrets
                    .iter()
                    .map(|(var, name)| format!("{var} ← 环境变量（钥匙串里是 {name}）"))
                    .collect();
                item.reason = format!(
                    "项目清单里声明了这个 MCP 服务，所以它对本项目生效。\n{tool_name} 项目里没有不提交的配置，方寸只写了从环境转发 {} 的引用，不替它写明文；启动 {tool_name} 前要在环境里设好。",
                    vars.iter()
                        .map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join("、")
                );
            }
        }
    }
    // 项目层那条只是引用，被个人层带明文的那条整条盖过：并进它的「在哪一层声明」，不单列。
    context
        .items
        .retain(|i| !i.overridden_by.as_ref().is_some_and(|w| folded.contains(w)));
}

/// 写入了哪些文件（6.3 检查器第 5 条）。
fn written(item: &EffectiveItem, names: &[String], ledger: &Ledger, library: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for owned in ledger.paths.iter().filter(|p| names.contains(&p.asset)) {
        let relevant = match owned.kind {
            PathKind::Mirror => item.kind == AssetKind::Instruction,
            _ => item.kind == AssetKind::Skill,
        };
        if !relevant {
            continue;
        }
        let line = match (owned.kind, &owned.target) {
            (PathKind::Mirror, _) => {
                format!("{} · 方寸生成的镜像", owned.path)
            }
            (PathKind::SkillDir, _) => format!("{} · 入库的实体目录", owned.path),
            (_, Some(target)) if Path::new(target).starts_with(library) => {
                format!("{} → 资产库", owned.path)
            }
            (_, Some(target)) => format!("{} → {target}", owned.path),
            (_, None) => owned.path.clone(),
        };
        out.push(line);
    }
    for key in ledger.keys.iter().filter(|k| names.contains(&k.asset)) {
        let shown = key.key.replace("{project}", "<本项目>");
        let line = match key.kind {
            KeyKind::Mcp if item.kind == AssetKind::Mcp => {
                format!("{} → {shown}（按键合并）", key.file)
            }
            KeyKind::McpPersonal if item.kind == AssetKind::Mcp => {
                format!("{} → {shown}（本机个人层，按键合并）", key.file)
            }
            KeyKind::SkillOff if item.kind == AssetKind::Skill => {
                format!("{} → {shown}（在本项目停用）", key.file)
            }
            KeyKind::McpOff | KeyKind::McpField if item.kind == AssetKind::Mcp => {
                format!("{} → {shown}（在本项目停用）", key.file)
            }
            _ => continue,
        };
        out.push(line);
    }
    // 合成一行的十几项各写一行太长：同一种写法只留一行，写明几项。
    if names.len() > 1 && out.len() > 3 {
        let mut grouped: BTreeMap<String, usize> = BTreeMap::new();
        for line in &out {
            let tail = line.rsplit_once('（').map_or("", |(_, t)| t);
            let file = line.split(" → ").next().unwrap_or_default();
            *grouped
                .entry(format!("{file} · {}", tail.trim_end_matches('）')))
                .or_default() += 1;
        }
        return grouped
            .into_iter()
            .map(|(k, n)| format!("{k} · {n} 项"))
            .collect();
    }
    out
}

/// 最近活动（6.3 检查器第 6 条）：记录里提到这一项的名字（或它用的密钥）的，新的在前。
fn recent(item: &EffectiveItem, names: &[String], records: &[Record]) -> Vec<ItemActivity> {
    let mut wanted: Vec<String> = names.to_vec();
    // 「GITHUB_TOKEN → 钥匙串 github-token」里的密钥名称：密钥记录也算这一项的。
    for line in &item.secrets {
        if let Some(name) = line.rsplit(' ').next() {
            wanted.push(name.trim_end_matches('）').to_owned());
        }
    }
    let hits = |text: &str| {
        text.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == '@'))
            .flat_map(|t| t.split('@').next())
            .any(|token| wanted.iter().any(|w| w == token))
    };
    let mut out = Vec::new();
    for record in records {
        let row = record
            .files
            .iter()
            .find(|f| hits(&f.path) || f.note.as_deref().is_some_and(&hits));
        let text = match row {
            Some(f) => match &f.note {
                Some(note) if !note.is_empty() => format!("{} · {note}", record.title),
                _ => format!("{} · {}", record.title, f.path),
            },
            None if hits(&record.title) => record.title.clone(),
            None => continue,
        };
        out.push(ItemActivity {
            record: record.id.clone(),
            at: record.at.clone(),
            kind: record.kind,
            text,
        });
        if out.len() >= RECENT_LIMIT {
            break;
        }
    }
    out
}
