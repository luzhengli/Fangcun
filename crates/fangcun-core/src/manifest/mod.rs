//! 清单、锁、套装文件的读写（F3）。文件即真相：界面只是它们的编辑器。
//!
//! | 文件 | 位置 | 类型 |
//! | --- | --- | --- |
//! | `fangcun.toml` | 项目根目录，提交 | [`Manifest`] |
//! | `fangcun.local.toml` | 项目根目录，不提交 | [`Manifest`]（个人覆盖） |
//! | `fangcun.lock` | 项目根目录，提交 | [`Lock`] |
//! | `<名称>.toml` | `~/.fangcun/bundles/` | [`Bundle`] |
//!
//! 界面改清单时要保住人写的注释和顺序。这里的 serde 结构只负责读；
//! 写回用保留格式的编辑方式，实现 M1 的清单编辑时再引入。

mod edit;
mod interop;
mod lock;
mod render;
mod schema;

pub use edit::{ManifestEdit, apply_edits, remove_skill, set_skill_pin};
pub use interop::{
    ExportedSkillsLock, SkillsLock, SkillsLockEntry, export_skills_lock, read_skills_lock,
};
pub use lock::{LOCK_VERSION, Lock, LockedInstruction, LockedMcp, LockedSkill};
pub use render::render_manifest;
pub use schema::{
    Bundle, BundleSection, DevSection, DisableSection, InstructionsDecl, Manifest, McpDecl,
    ProjectSection, SkillDecl, glob,
};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::model::{SyncMode, ToolId};
use crate::paths::FangcunHome;
use crate::{Error, Result};

/// 解析一份清单文本。`origin` 只用于报错。
pub fn parse_manifest(origin: &str, text: &str) -> Result<Manifest> {
    toml::from_str(text).map_err(|e| Error::Parse {
        origin: origin.to_owned(),
        message: e.to_string(),
    })
}

/// 项目清单 `[project]` 里项目页要显示的几个键。
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
pub struct ProjectBasics {
    /// 常驻预算。没写时 `None`，调用方用默认值（D-22）。
    pub budget: Option<u32>,
    /// 物化方式。没写时 `None`，同步按链接（`materializer::sync`）。
    pub mode: Option<SyncMode>,
    #[serde(default)]
    pub targets: Vec<ToolId>,
    /// 清单的 `[disable]`。
    #[serde(skip)]
    pub disable: DisableSection,
}

/// 读项目清单 `[project]` 的预算、物化方式、目标工具。没有清单时返回 `None`。
///
/// 只读这几个键，不因为清单别处的键报错：项目页能显示，校验留给同步。
pub fn project_basics(project_root: &Path) -> Result<Option<ProjectBasics>> {
    #[derive(serde::Deserialize)]
    struct Partial {
        #[serde(default)]
        project: ProjectBasics,
        #[serde(default)]
        disable: Option<toml::Value>,
    }

    let path = project_root.join(crate::paths::MANIFEST_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(Error::Io { path, source }),
    };
    let partial: Partial = toml::from_str(&text).map_err(|e| Error::Parse {
        origin: path.display().to_string(),
        message: e.to_string(),
    })?;
    let mut basics = partial.project;
    // `[disable]` 写错了不挡项目页：读不懂就当没写，同步时再报。
    basics.disable = partial
        .disable
        .and_then(|v| v.try_into().ok())
        .unwrap_or_default();
    Ok(Some(basics))
}

/// 一个项目的全部声明文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: Manifest,
    /// `fangcun.local.toml`，没有时为空。
    pub local: Option<Manifest>,
    /// 清单引用的套装，按清单里的顺序。本机没有这个套装文件时 `bundle` 为空（7.4：按锁还原）。
    pub bundles: Vec<LoadedBundle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedBundle {
    pub name: String,
    pub bundle: Option<Bundle>,
}

/// 读项目的清单、个人覆盖、引用的套装。项目没有清单时返回 `None`。
///
/// 每份文件只能写自己的节（7.4），写错了报错：清单不能写 `[dev]`、`[bundle]`；个人覆盖不能改
/// `targets`、`mode`、`budget`，不能写 `[bundle]`；套装文件不能写 `[project]`、`[dev]`。
pub fn load_project(project_root: &Path, home: &FangcunHome) -> Result<Option<Project>> {
    let Some(manifest) = read_manifest(&project_root.join(crate::paths::MANIFEST_FILE))? else {
        return Ok(None);
    };
    let origin = crate::paths::MANIFEST_FILE;
    if !manifest.dev.is_empty() {
        return Err(misplaced(origin, "[dev] 只能写在 fangcun.local.toml 里"));
    }
    if manifest.bundle.is_some() {
        return Err(misplaced(origin, "[bundle] 只能写在套装文件里"));
    }

    let local = read_manifest(&project_root.join(crate::paths::LOCAL_MANIFEST_FILE))?;
    if let Some(local) = &local {
        let origin = crate::paths::LOCAL_MANIFEST_FILE;
        let project = &local.project;
        if !project.targets.is_empty() || project.mode.is_some() || project.budget.is_some() {
            return Err(misplaced(
                origin,
                "个人覆盖不能改 targets、mode、budget，这些是全队共享的设置",
            ));
        }
        if local.bundle.is_some() {
            return Err(misplaced(origin, "[bundle] 只能写在套装文件里"));
        }
    }

    let mut bundles = Vec::new();
    let names = manifest
        .project
        .bundles
        .iter()
        .chain(local.iter().flat_map(|l| l.project.bundles.iter()));
    for name in names {
        if bundles.iter().any(|b: &LoadedBundle| &b.name == name) {
            continue;
        }
        let file = home.bundle_file(name);
        let bundle = read_manifest(&file)?;
        if let Some(bundle) = &bundle {
            let origin = file.display().to_string();
            if bundle.project != ProjectSection::default() || !bundle.dev.is_empty() {
                return Err(misplaced(&origin, "套装文件不能写 [project]、[dev]"));
            }
        }
        bundles.push(LoadedBundle {
            name: name.clone(),
            bundle,
        });
    }

    Ok(Some(Project {
        root: project_root.to_path_buf(),
        manifest,
        local,
        bundles,
    }))
}

fn read_manifest(path: &Path) -> Result<Option<Manifest>> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_manifest(&path.display().to_string(), &text).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn misplaced(origin: &str, message: &str) -> Error {
    Error::Parse {
        origin: origin.to_owned(),
        message: message.to_owned(),
    }
}

/// 一项声明来自哪一层（4.3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Layer {
    Bundle(String),
    Project,
    Personal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared<T> {
    pub decl: T,
    pub layer: Layer,
}

/// 合并后的声明：下层覆盖上层，项目清单盖过套装，个人覆盖盖过项目清单（4.3、7.4）。
/// 同一项出现在两个套装里时，清单里排在后面的套装盖过前面的。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declarations {
    pub skills: BTreeMap<String, Declared<SkillDecl>>,
    pub mcp: BTreeMap<String, Declared<McpDecl>>,
    pub instructions: Option<Declared<InstructionsDecl>>,
    /// 清单与个人覆盖的 `[disable]` 合在一起。
    pub disable: DisableSection,
    pub dev: Vec<String>,
}

impl Project {
    /// 全队共享的声明：套装与项目清单，不含个人覆盖。写锁用它，个人加的项不进锁（7.4）。
    pub fn team_declarations(&self) -> Declarations {
        self.declarations(false)
    }

    /// 本机生效的声明：再叠上个人覆盖。同步（M1-10）用它。
    pub fn local_declarations(&self) -> Declarations {
        self.declarations(true)
    }

    fn declarations(&self, personal: bool) -> Declarations {
        let mut out = Declarations::default();
        let mut layers: Vec<(&Manifest, Layer)> = self
            .bundles
            .iter()
            .filter_map(|b| {
                b.bundle
                    .as_ref()
                    .map(|m| (m, Layer::Bundle(b.name.clone())))
            })
            .collect();
        layers.push((&self.manifest, Layer::Project));
        if personal && let Some(local) = &self.local {
            layers.push((local, Layer::Personal));
        }
        for (manifest, layer) in layers {
            for (name, decl) in &manifest.skills {
                out.skills.insert(
                    name.clone(),
                    Declared {
                        decl: decl.clone(),
                        layer: layer.clone(),
                    },
                );
            }
            for (name, decl) in &manifest.mcp {
                out.mcp.insert(
                    name.clone(),
                    Declared {
                        decl: decl.clone(),
                        layer: layer.clone(),
                    },
                );
            }
            if let Some(decl) = &manifest.instructions {
                out.instructions = Some(Declared {
                    decl: decl.clone(),
                    layer: layer.clone(),
                });
            }
            if layer != Layer::Personal || personal {
                extend_unique(&mut out.disable.skills, &manifest.disable.skills);
                extend_unique(&mut out.disable.mcp, &manifest.disable.mcp);
            }
            extend_unique(&mut out.dev, &manifest.dev.skills);
        }
        out
    }
}

fn extend_unique(into: &mut Vec<String>, from: &[String]) {
    for item in from {
        if !into.contains(item) {
            into.push(item.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SyncMode;

    /// 设计文档 7.4 的清单示例，原样照抄。示例改了，这里跟着改。
    const DESIGN_DOC_EXAMPLE: &str = r#"
# fangcun.toml —— 提交到 git
[project]
targets = ["claude-code", "codex"]
mode    = "link"                 # link 或 vendor
budget  = 20000                  # 常驻预算（估算值），不写时 20000

bundles = ["工程基线"]

[skills]
tdd             = { source = "github:obra/superpowers", track = "main" }
skill-b         = { source = "github:acme/skills", pin = "v2.1.0" }
project-harness = { source = "local:luyao-skills", pin = "4d9b1eb" }

[mcp.github]
command = "npx"
args    = ["-y", "@modelcontextprotocol/server-github"]
env     = { GITHUB_TOKEN = "secret:github-token" }   # 值在钥匙串

[instructions]
canonical = "AGENTS.md"          # 正本
mirror    = ["CLAUDE.md"]        # 生成为对正本的引用
"#;

    #[test]
    fn parses_the_design_doc_example() {
        let manifest = parse_manifest("7.4", DESIGN_DOC_EXAMPLE).expect("7.4 的示例应能解析");

        assert_eq!(manifest.project.targets.len(), 2);
        assert_eq!(manifest.project.mode, Some(SyncMode::Link));
        assert_eq!(manifest.project.bundles, ["工程基线"]);
        assert_eq!(manifest.project.budget, Some(20_000));

        assert_eq!(manifest.skills.len(), 3);
        assert_eq!(manifest.skills["tdd"].track.as_deref(), Some("main"));
        assert_eq!(manifest.skills["skill-b"].pin.as_deref(), Some("v2.1.0"));

        let github = &manifest.mcp["github"];
        assert_eq!(github.command, "npx");
        assert_eq!(github.env["GITHUB_TOKEN"], "secret:github-token");

        let instructions = manifest.instructions.expect("示例里有指令文件一节");
        assert_eq!(instructions.canonical, "AGENTS.md");
        assert_eq!(instructions.mirror, ["CLAUDE.md"]);
    }

    #[test]
    fn project_basics_reads_only_its_keys() {
        let root = std::env::temp_dir().join(format!("fangcun-budget-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("建临时目录");
        assert_eq!(project_basics(&root).expect("没有清单"), None);
        let manifest = root.join(crate::paths::MANIFEST_FILE);
        std::fs::write(&manifest, "[project]\nbudget = 12000\nlater = true\n").expect("写清单");
        let basics = project_basics(&root).expect("读预算").expect("有清单");
        assert_eq!(basics.budget, Some(12_000));
        assert_eq!(basics.mode, None);
        assert!(basics.targets.is_empty());
        std::fs::write(
            &manifest,
            "[project]\ntargets = [\"claude-code\", \"codex\"]\nmode = \"vendor\"\n",
        )
        .expect("写清单");
        let basics = project_basics(&root).expect("读清单").expect("有清单");
        assert_eq!(basics.mode, Some(SyncMode::Vendor));
        assert_eq!(basics.targets.len(), 2);
        std::fs::write(&manifest, "[project]\nbudget = \"12k\"\n").expect("写清单");
        assert!(project_basics(&root).is_err(), "写错了要报出来");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        assert!(parse_manifest("test", "[project]\ntargets = []\nsurprise = 1\n").is_err());
    }
}
