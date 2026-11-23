//! 项目页的清单页签（6.3「清单页签」、M1-31）：三份文件各一张卡，结构化视图按区块列表格，
//! 源文件视图是原文。只读。
//!
//! 密钥只给名称和在不在钥匙串里；文件里写成明文的值在两种视图里都打码（规则 9 的精神：
//! 界面上不出现密钥的值）。

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Result;
use crate::manifest::{Layer, Lock, Manifest, McpDecl, Project, SkillDecl};
use crate::model::SyncMode;
use crate::secrets::{self, SecretStore};

/// 清单页签的三张文件卡。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ManifestView {
    pub files: Vec<ManifestFile>,
}

/// 一份文件：`fangcun.toml`、`fangcun.lock` 或 `fangcun.local.toml`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ManifestFile {
    pub name: String,
    /// 角色，例如「项目清单」。
    pub role: String,
    /// 提不提交，例如「提交到 git」。
    pub tag: String,
    pub committed: bool,
    /// 文件在不在。
    pub exists: bool,
    /// 顶部一句用途。
    pub description: String,
    pub sections: Vec<ManifestSection>,
    /// 源文件，一行一项。明文密钥打码。
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ManifestSection {
    pub title: String,
    /// 三列的表头。
    pub headers: Vec<String>,
    pub rows: Vec<ManifestRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ManifestRow {
    /// 三格。
    pub cells: Vec<String>,
    /// 哪几格用等宽字体。
    pub mono: Vec<bool>,
    /// 第三格下面的小字，例如「来自套装「工程基线」」。
    pub note: Option<String>,
    /// 第三格标橙：停用、钥匙串里没有、明文。
    pub warn: bool,
}

fn row(cells: [&str; 3], mono: [bool; 3], note: Option<String>, warn: bool) -> ManifestRow {
    ManifestRow {
        cells: cells.iter().map(|c| (*c).to_owned()).collect(),
        mono: mono.to_vec(),
        note,
        warn,
    }
}

fn section(title: &str, headers: [&str; 3], rows: Vec<ManifestRow>) -> ManifestSection {
    ManifestSection {
        title: title.to_owned(),
        headers: headers.iter().map(|h| (*h).to_owned()).collect(),
        rows,
    }
}

/// 明文打码后的源文件行。
fn source_lines(root: &Path, file: &str, plain: &[String]) -> Vec<String> {
    let Ok(mut text) = std::fs::read_to_string(root.join(file)) else {
        return Vec::new();
    };
    for value in plain {
        text = text.replace(value.as_str(), "••••••");
    }
    text.lines().map(str::to_owned).collect()
}

/// 文件里看起来像明文密钥的值。
fn plaintexts(manifest: Option<&Manifest>) -> Vec<String> {
    manifest
        .into_iter()
        .flat_map(|m| m.mcp.values())
        .flat_map(|decl| decl.env.iter())
        .filter(|(k, v)| crate::scanner::looks_like_secret(k, v))
        .map(|(_, v)| v.clone())
        .collect()
}

fn policy(decl: &SkillDecl) -> String {
    match (&decl.pin, &decl.track) {
        (Some(pin), _) => format!("固定 {pin}"),
        (None, Some(track)) => format!("跟随 {track} 分支"),
        (None, None) if decl.source.starts_with("local:") => "本地工作目录现状".to_owned(),
        (None, None) => "默认分支最新".to_owned(),
    }
}

fn short(hash: &str) -> String {
    let n = hash.chars().count();
    if n <= 8 {
        return format!("sha256:{hash}");
    }
    let head: String = hash.chars().take(4).collect();
    let tail: String = hash.chars().skip(n - 4).collect();
    format!("sha256:{head}…{tail}")
}

/// MCP 一行的「密钥」格：用到的密钥在不在钥匙串里。
fn mcp_secret_cell(decl: &McpDecl, store: &dyn SecretStore) -> (String, Option<String>, bool) {
    let mut parts = Vec::new();
    let mut notes = Vec::new();
    let mut warn = false;
    for (var, value) in &decl.env {
        if let Some(name) = secrets::reference(value) {
            match store.get(name) {
                Ok(Some(_)) => {
                    parts.push(format!("{var} 存在钥匙串"));
                    notes.push(format!("文件里只写引用 secret:{name}"));
                }
                Ok(None) => {
                    warn = true;
                    parts.push(format!("{var} 钥匙串里没有"));
                    notes.push(format!("先运行 fangcun secret set {name}"));
                }
                Err(e) => {
                    warn = true;
                    parts.push(format!("{var} 读不到钥匙串"));
                    notes.push(e.to_string());
                }
            }
        } else if crate::scanner::looks_like_secret(var, value) {
            warn = true;
            parts.push(format!("{var} 是明文"));
            notes.push("改成 secret: 引用，值存进钥匙串".to_owned());
        }
    }
    if parts.is_empty() {
        return ("不用密钥".to_owned(), None, false);
    }
    let note = (!notes.is_empty()).then(|| notes.join("；"));
    (parts.join("、"), note, warn)
}

fn command_line(decl: &McpDecl) -> String {
    std::iter::once(decl.command.as_str())
        .chain(decl.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn build(
    project: &Project,
    lock: Option<&Lock>,
    store: &dyn SecretStore,
    adapters: &crate::adapter::AdapterRegistry,
) -> Result<ManifestView> {
    let root = project.root.as_path();
    let manifest = &project.manifest;
    let team = project.team_declarations();

    // ── fangcun.toml ──
    let names: Vec<String> = manifest
        .project
        .targets
        .iter()
        .map(|t| t.as_str().to_owned())
        .collect();
    let tool_names: Vec<String> = manifest
        .project
        .targets
        .iter()
        .map(|t| {
            adapters
                .get(t)
                .map_or(t.as_str(), |a| a.tool.name.as_str())
                .to_owned()
        })
        .collect();
    let mut settings = vec![row(
        [
            "目标工具",
            &names.join(", "),
            &format!("按本清单同步写入 {} 的原生配置", tool_names.join("、")),
        ],
        [false, true, false],
        None,
        false,
    )];
    let mode = manifest.project.mode.unwrap_or(SyncMode::Link);
    settings.push(match (manifest.project.mode, mode) {
        (_, SyncMode::Link) => row(
            [
                "同步方式",
                "link",
                "本机软链：项目目录放软链指向本机资产库，不进 Git",
            ],
            [false, true, false],
            Some(if manifest.project.mode.is_none() {
                "清单未指定，默认使用 link（本机软链）。团队协作可改为 vendor（拷入仓库）"
                    .to_owned()
            } else {
                "团队协作可改为 vendor（拷入仓库），随 Git 提交".to_owned()
            }),
            false,
        ),
        (_, SyncMode::Vendor) => row(
            [
                "同步方式",
                "vendor",
                "拷入仓库：实体文件复制进项目目录，随 Git 提交",
            ],
            [false, true, false],
            None,
            false,
        ),
    });
    if !manifest.project.bundles.is_empty() {
        let missing: Vec<&str> = project
            .bundles
            .iter()
            .filter(|b| b.bundle.is_none())
            .map(|b| b.name.as_str())
            .collect();
        settings.push(row(
            [
                "套装",
                &manifest.project.bundles.join("、"),
                "套装里的资产一并生效",
            ],
            [false, false, false],
            (!missing.is_empty())
                .then(|| format!("本机没有套装「{}」，按锁还原", missing.join("」「"))),
            !missing.is_empty(),
        ));
    }
    if let Some(budget) = manifest.project.budget {
        settings.push(row(
            ["常驻预算", &budget.to_string(), "tokens，估算值"],
            [false, true, false],
            None,
            false,
        ));
    }

    let mut skills: Vec<ManifestRow> = team
        .skills
        .iter()
        .map(|(name, item)| {
            let note = match &item.layer {
                Layer::Bundle(bundle) => Some(format!("来自套装「{bundle}」")),
                _ => None,
            };
            row(
                [name, &item.decl.source, &policy(&item.decl)],
                [true, true, false],
                note,
                false,
            )
        })
        .collect();
    for pattern in &manifest.disable.skills {
        skills.push(row(
            [pattern, "全局层", "本项目停用"],
            [true, false, false],
            None,
            true,
        ));
    }

    let mut mcp: Vec<ManifestRow> = team
        .mcp
        .iter()
        .map(|(name, item)| {
            let (cell, note, warn) = mcp_secret_cell(&item.decl, store);
            let note = match (&item.layer, note) {
                (Layer::Bundle(bundle), Some(n)) => Some(format!("来自套装「{bundle}」；{n}")),
                (Layer::Bundle(bundle), None) => Some(format!("来自套装「{bundle}」")),
                (_, n) => n,
            };
            row(
                [name, &command_line(&item.decl), &cell],
                [true, true, false],
                note,
                warn,
            )
        })
        .collect();
    for pattern in &manifest.disable.mcp {
        mcp.push(row(
            [pattern, "全局层", "本项目停用"],
            [true, false, false],
            None,
            true,
        ));
    }

    let mut instructions = Vec::new();
    if let Some(item) = &team.instructions {
        instructions.push(row(
            ["正本", &item.decl.canonical, "手工编辑这一份"],
            [false, true, false],
            None,
            false,
        ));
        for mirror in &item.decl.mirror {
            instructions.push(row(
                [
                    "镜像",
                    mirror,
                    &format!("自动生成，内容是对 {} 的引用", item.decl.canonical),
                ],
                [false, true, false],
                None,
                false,
            ));
        }
    }

    let mut sections = vec![section("项目设置", ["设置", "值", "含义"], settings)];
    for (title, headers, rows) in [
        ("Skills", ["名称", "来源", "版本策略"], skills),
        ("MCP 服务", ["名称", "启动命令", "密钥"], mcp),
        ("指令文件", ["角色", "文件", "说明"], instructions),
    ] {
        if !rows.is_empty() {
            sections.push(section(title, headers, rows));
        }
    }
    let toml = ManifestFile {
        name: crate::paths::MANIFEST_FILE.to_owned(),
        role: "项目清单".to_owned(),
        tag: "提交到 git".to_owned(),
        committed: true,
        exists: true,
        description: "项目清单。同事克隆仓库后运行 fangcun sync，就能还原完全相同的环境。界面上的改动最终都写回这个文件。".to_owned(),
        sections,
        lines: source_lines(root, crate::paths::MANIFEST_FILE, &plaintexts(Some(manifest))),
    };

    // ── fangcun.lock ──
    let mut locked = Vec::new();
    if let Some(lock) = lock {
        for (name, skill) in &lock.skills {
            let commit = skill
                .commit
                .as_deref()
                .map(crate::scanner::short_hash)
                .unwrap_or_default();
            let resolved = match (&skill.pin, &skill.track, commit.is_empty()) {
                (_, _, true) => "本地工作目录现状".to_owned(),
                (Some(pin), _, _) if *pin == commit || pin.starts_with(&commit) => commit,
                (Some(pin), _, _) => format!("{pin} → {commit}"),
                (None, Some(track), _) => format!("{track} → {commit}"),
                (None, None, _) => commit,
            };
            let note = skill.bundle.as_ref().map(|b| format!("来自套装「{b}」"));
            locked.push(row(
                [name, &resolved, &short(&skill.hash.0)],
                [true, true, true],
                note,
                false,
            ));
        }
        for (name, entry) in &lock.mcp {
            locked.push(row(
                [name, "启动命令", &short(&entry.hash.0)],
                [true, false, true],
                Some("启动命令一变，升级时会高亮提示".to_owned()),
                false,
            ));
        }
        for (name, entry) in &lock.instructions {
            locked.push(row(
                [name, "正本内容", &short(&entry.hash.0)],
                [true, false, true],
                None,
                false,
            ));
        }
    }
    let lock_file = ManifestFile {
        name: crate::paths::LOCK_FILE.to_owned(),
        role: "版本锁".to_owned(),
        tag: "提交 · 自动生成".to_owned(),
        committed: true,
        exists: lock.is_some(),
        description: "由方寸根据清单解析生成，记录每项资产确切的提交与内容哈希。每次同步都会校验哈希，来源被篡改会被拦下。不要手工编辑。".to_owned(),
        sections: if locked.is_empty() {
            Vec::new()
        } else {
            vec![section("已解析的版本", ["资产", "解析到", "内容哈希"], locked)]
        },
        lines: source_lines(root, crate::paths::LOCK_FILE, &[]),
    };

    // ── fangcun.local.toml ──
    let mut personal = Vec::new();
    if let Some(local) = &project.local {
        let dev: &[String] = &local.dev.skills;
        for (name, decl) in &local.skills {
            let label = format!("{name} · {}", decl.source);
            let replaces = manifest.skills.contains_key(name);
            personal.push(row(
                [
                    if replaces {
                        "改版本"
                    } else {
                        "追加 skill"
                    },
                    &label,
                    &policy(decl),
                ],
                [false, true, false],
                Some("只在你的机器上加载".to_owned()),
                false,
            ));
        }
        for name in dev {
            personal.push(row(
                ["开发模式", name, "直接链到本地来源的工作目录，改了立即可见"],
                [false, true, false],
                None,
                false,
            ));
        }
        for (name, decl) in &local.mcp {
            let (cell, note, warn) = mcp_secret_cell(decl, store);
            let replaces = manifest.mcp.contains_key(name);
            personal.push(row(
                [
                    if replaces { "改 MCP" } else { "追加 MCP" },
                    &format!("{name} · {}", command_line(decl)),
                    &format!("写进工具的个人层，不提交 · {cell}"),
                ],
                [false, true, false],
                note,
                warn,
            ));
        }
        for (kind, patterns) in [
            ("skill", &local.disable.skills),
            ("MCP", &local.disable.mcp),
        ] {
            for pattern in patterns {
                personal.push(row(
                    ["停用", pattern, &format!("你本人不加载这个 {kind}")],
                    [false, true, false],
                    None,
                    true,
                ));
            }
        }
    }
    let local_file = ManifestFile {
        name: crate::paths::LOCAL_MANIFEST_FILE.to_owned(),
        role: "个人覆盖".to_owned(),
        tag: "不提交".to_owned(),
        committed: false,
        exists: project.local.is_some(),
        description: "只属于你的覆盖，已自动加入 .gitignore。它优先于项目清单，但只在你的机器上生效，同事看不到。".to_owned(),
        sections: if personal.is_empty() {
            Vec::new()
        } else {
            vec![section("个人覆盖", ["项", "值", "效果"], personal)]
        },
        lines: source_lines(root, crate::paths::LOCAL_MANIFEST_FILE, &plaintexts(project.local.as_ref())),
    };

    Ok(ManifestView {
        files: vec![toml, lock_file, local_file],
    })
}
