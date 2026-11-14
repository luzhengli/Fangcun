//! 适配器的回归样例（7.3）：每个工具一棵仿真的主目录和项目目录，在 `adapters/fixtures/<工具>/`。
//! 工具升级、适配器改动后跑这里，就知道读取位置和优先级规则还对不对。
//!
//! 样例复制到 `target/` 下的临时目录再扫：会话记录要写入真实的绝对路径，
//! 而且扫描器把系统临时目录当作「不是项目」。

use std::fs;
use std::path::{Path, PathBuf};

use fangcun_core::journal::RecordFilter;
use fangcun_core::model::{AssetKind, AssetState, ObservedKind, Scope, Timestamp, ToolId};
use fangcun_core::paths::FangcunHome;
use fangcun_core::resolver::EffectiveContext;
use fangcun_core::scanner::FindingCategory;
use fangcun_core::{Access, Kernel};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../adapters/fixtures");

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("建目录");
    for entry in fs::read_dir(from).expect("读样例目录").flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("复制样例文件");
        }
    }
}

/// 把一个工具的样例复制到干净的目录，返回（主目录，项目目录）。
fn stage(tool: &str) -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("adapter-{tool}"));
    let _ = fs::remove_dir_all(&root);
    // 项目放在主目录里，和真实机器一样：Claude Code 往上找指令文件时会走到主目录。
    let home = root.join("home");
    let project = home.join("code").join("demo");
    copy_tree(&Path::new(FIXTURES).join(tool).join("home"), &home);
    copy_tree(&Path::new(FIXTURES).join(tool).join("project"), &project);
    fs::create_dir_all(project.join(".git")).expect("建 .git");
    (home, project)
}

fn kernel(home: &Path, project: &Path) -> Kernel {
    Kernel::with_home(FangcunHome::at(home.join(".fangcun")), Access::ReadOnly)
        .expect("打开内核")
        .with_user_home(home)
        .with_project(project)
}

/// 只读：方寸只写了 `~/.fangcun/journal` 一个目录，里面是体检记录（D-15）。
fn only_journal_written(home: &Path) {
    let entries: Vec<String> = fs::read_dir(home.join(".fangcun"))
        .expect("体检记录应已写入")
        .map(|e| {
            e.expect("目录项")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(entries, ["journal"]);
}

fn find<'a>(
    context: &'a EffectiveContext,
    name: &str,
    scope: Scope,
) -> &'a fangcun_core::resolver::EffectiveItem {
    context
        .items
        .iter()
        .find(|i| i.name == name && i.scope == scope)
        .unwrap_or_else(|| {
            panic!(
                "生效上下文里应有{scope:?}层的 {name}：{:#?}",
                names(context)
            )
        })
}

fn names(context: &EffectiveContext) -> Vec<String> {
    context
        .items
        .iter()
        .map(|i| format!("{:?} {:?} {} [{}]", i.kind, i.scope, i.name, i.label))
        .collect()
}

#[test]
fn claude_code_reads_what_the_docs_say() {
    let (home, project) = stage("claude-code");
    // 个人层 MCP 存在 ~/.claude.json 的 projects.<项目路径>.mcpServers 下，键里要有真实路径。
    let claude_json = serde_json::json!({
        "mcpServers": { "context7": { "command": "npx", "args": ["-y", "@upstash/context7-mcp"] } },
        "projects": {
            project.display().to_string(): {
                "mcpServers": { "github": { "command": "gh-mcp-local" } }
            }
        }
    });
    fs::write(home.join(".claude.json"), claude_json.to_string()).expect("写 .claude.json");
    // 插件的安装清单要写真实的安装目录。
    let plugin_dir = home.join(".claude/plugins/cache/market/code-review-kit/1.0.0");
    for skill in ["review", "explain"] {
        fs::create_dir_all(plugin_dir.join("skills").join(skill)).expect("建插件 skill");
        fs::write(
            plugin_dir.join("skills").join(skill).join("SKILL.md"),
            "---\nname: x\n---\n",
        )
        .expect("写插件 skill");
    }
    let installed = serde_json::json!({ "version": 2, "plugins": {
        "code-review-kit@market": [{ "scope": "user", "installPath": plugin_dir.display().to_string() }]
    } });
    fs::write(
        home.join(".claude/plugins/installed_plugins.json"),
        installed.to_string(),
    )
    .expect("写安装清单");

    // 一份会话记录：在项目里触发了 lark-doc，调用了 context7。
    let now = Timestamp::now().0;
    let cwd = project.display().to_string();
    let lines = [
        serde_json::json!({ "type": "user", "timestamp": now, "cwd": cwd, "message": { "content": "hi" } }),
        serde_json::json!({ "type": "assistant", "timestamp": now, "cwd": cwd, "message": { "content": [
            { "type": "tool_use", "name": "Skill", "input": { "skill": "lark-doc" } },
            { "type": "tool_use", "name": "mcp__context7__get_docs", "input": {} }
        ] } }),
    ];
    let sessions = home.join(".claude/projects/demo");
    fs::create_dir_all(&sessions).expect("建会话目录");
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    fs::write(sessions.join("s1.jsonl"), text.join("\n")).expect("写会话记录");

    let kernel = kernel(&home, &project);
    let context = kernel
        .effective_context(&project, &ToolId::from(ToolId::CLAUDE_CODE))
        .expect("解出生效上下文");

    // 指令：全局 CLAUDE.md 与项目 CLAUDE.md 都加载；有 CLAUDE.md 时不读 AGENTS.md；带 paths 的规则按需。
    // 项目 CLAUDE.md 里的 @docs/conventions.md 一起带进来，反引号里的不算。
    let project_md = find(&context, "CLAUDE.md", Scope::Project);
    assert_eq!(project_md.files.len(), 2, "{:?}", project_md.files);
    assert!(project_md.files[1].contains("conventions.md"));
    let global_md = context
        .items
        .iter()
        .find(|i| i.kind == AssetKind::Instruction && i.scope == Scope::Global)
        .expect("全局 CLAUDE.md");
    assert!(global_md.counted);
    let claude_files = context
        .items
        .iter()
        .filter(|i| i.location.ends_with(".claude/CLAUDE.md"))
        .count();
    assert_eq!(
        claude_files,
        1,
        "~/.claude/CLAUDE.md 只算在全局层：{:#?}",
        names(&context)
    );
    assert!(
        !context.items.iter().any(|i| i.name == "AGENTS.md"),
        "有 CLAUDE.md 时不读 AGENTS.md：{:#?}",
        names(&context)
    );
    let rule = context
        .items
        .iter()
        .find(|i| i.name.ends_with("testing.md"))
        .expect("规则文件");
    assert!(!rule.counted, "带 paths 的规则按需加载，不计入常驻");
    assert!(
        context
            .items
            .iter()
            .any(|i| i.name.ends_with("general.md") && i.counted)
    );

    // Skill：同名时全局层盖过项目层。
    let global_fd = find(&context, "frontend-design", Scope::Global);
    let project_fd = find(&context, "frontend-design", Scope::Project);
    assert_eq!(project_fd.state, Some(AssetState::Overridden));
    assert!(global_fd.counted && !project_fd.counted);
    let lark = find(&context, "lark-doc", Scope::Global);
    assert_eq!(lark.triggers, Some(1), "会话记录里触发过一次");

    // MCP：个人层 > 项目层 > 全局层，整条取用。
    let personal = find(&context, "github", Scope::Personal);
    let shared = find(&context, "github", Scope::Project);
    assert!(personal.counted);
    assert_eq!(shared.state, Some(AssetState::Overridden));
    assert_eq!(find(&context, "context7", Scope::Global).triggers, Some(1));
    assert!(context.budget.mcp_unestimated >= 2, "MCP 不估算常驻");

    // 其他加载项：启用的插件、项目的 Hook、斜杠命令；停用的插件不算。
    let observed: Vec<(ObservedKind, &str)> = context
        .observed
        .iter()
        .map(|o| (o.kind, o.name.as_str()))
        .collect();
    assert!(observed.contains(&(ObservedKind::Plugin, "code-review-kit@market")));
    let plugin = context
        .observed
        .iter()
        .find(|o| o.name == "code-review-kit@market")
        .expect("插件");
    assert_eq!(plugin.note.as_deref(), Some("含 2 个 skill"));
    assert!(!observed.iter().any(|(_, n)| n.starts_with("disabled-kit")));
    assert!(observed.contains(&(ObservedKind::Hook, "PostToolUse · Edit|Write")));
    assert!(observed.contains(&(ObservedKind::SlashCommand, "/release")));
    assert!(context.observed_missing.contains(&ObservedKind::Subagent));

    // 体检：项目里会提交的 .mcp.json 有明文密钥。
    let report = kernel.doctor().expect("体检");
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.category == FindingCategory::Secret)
    );
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.id == "duplicate:frontend-design")
    );

    only_journal_written(&home);
    assert_eq!(
        kernel.log(&RecordFilter::default()).expect("读记录").len(),
        1
    );
}

#[test]
fn codex_reads_what_the_docs_say() {
    let (home, project) = stage("codex");
    let kernel = kernel(&home, &project);
    let context = kernel
        .effective_context(&project, &ToolId::from(ToolId::CODEX))
        .expect("解出生效上下文");

    // 指令：全局 AGENTS.md；项目里 AGENTS.override.md 优先，同一层只取一份。
    assert!(
        context
            .items
            .iter()
            .any(|i| i.kind == AssetKind::Instruction && i.scope == Scope::Global)
    );
    find(&context, "AGENTS.override.md", Scope::Project);
    assert!(
        !context
            .items
            .iter()
            .any(|i| i.name == "AGENTS.md" && i.scope == Scope::Project)
    );

    // Skill：同名不合并，两份都加载。
    let global_tdd = find(&context, "tdd", Scope::Global);
    let project_tdd = find(&context, "tdd", Scope::Project);
    assert!(global_tdd.counted && project_tdd.counted);
    assert_eq!(global_tdd.source.as_deref(), Some("obra/superpowers"));
    // [[skills.config]] 按名称停用了 grill-me。
    let grill = find(&context, "grill-me", Scope::Global);
    assert_eq!(grill.label, "已停用");
    assert!(!grill.counted);

    // MCP：停用的不计入。项目没有标为受信任，项目里的 .codex/config.toml 不读。
    let context7 = find(&context, "context7", Scope::Global);
    assert!(!context7.counted);
    assert!(find(&context, "playwright", Scope::Global).counted);
    let untrusted = find(&context, "playwright", Scope::Project);
    assert!(!untrusted.counted);
    assert!(untrusted.reason.contains("受信任"));

    // 体检：锁与目录脱节（lark-vc 只在锁里，grill-me 只在目录里）。
    let report = kernel.doctor().expect("体检");
    let detached = report
        .findings
        .iter()
        .find(|f| f.category == FindingCategory::Detached)
        .expect("脱节发现");
    let paths: Vec<&str> = detached.locations.iter().map(|l| l.path.as_str()).collect();
    assert!(paths.contains(&"lark-vc") && paths.contains(&"grill-me"));
    only_journal_written(&home);

    // 标为受信任后：同名按字段合并，项目层只写了 args，command 沿用用户层。
    let mut config = fs::read_to_string(home.join(".codex/config.toml")).expect("读用户配置");
    config.push_str(&format!(
        "\n[projects.\"{}\"]\ntrust_level = \"trusted\"\n",
        project.display()
    ));
    fs::write(home.join(".codex/config.toml"), config).expect("写用户配置");
    let trusted = self::kernel(&home, &project);
    let context = trusted
        .effective_context(&project, &ToolId::from(ToolId::CODEX))
        .expect("解出生效上下文");
    let global_pw = find(&context, "playwright", Scope::Global);
    let project_pw = find(&context, "playwright", Scope::Project);
    assert_eq!(global_pw.state, Some(AssetState::Overridden));
    assert!(project_pw.counted && !global_pw.counted);
    assert_eq!(
        project_pw.command.as_deref(),
        Some("npx @playwright/mcp@1.0.0")
    );
}
