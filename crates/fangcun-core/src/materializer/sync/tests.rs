//! 同步、回滚、收编的测试：临时目录里造主目录、项目和冒充 GitHub 的 git 仓库，
//! 经 `Kernel` 从清单一直走到磁盘，再回滚回去逐字节比对。

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Error;
use crate::journal::{RecordFilter, RecordKind, RecordStatus};
use crate::kernel::{Access, Kernel};
use crate::model::{NoteTone, SnapshotId};
use crate::paths::FangcunHome;

struct Fixture {
    base: PathBuf,
    home: FangcunHome,
    user_home: PathBuf,
    proj: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

const TDD: &str = "---\nname: tdd\n---\nv1\n";

/// 上游 acme/skills 有 tdd（带一个可执行脚本）和 review。项目已登记，带 `.git`、
/// 别人写的 `.mcp.json`、`.codex/config.toml`、`.gitignore`。
fn fixture(name: &str) -> Fixture {
    let base = std::env::temp_dir().join(format!("fangcun-sync-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let work = base.join("work");
    fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    write(&work.join("skills/tdd/SKILL.md"), TDD);
    write(&work.join("skills/tdd/scripts/run.sh"), "echo hi\n");
    fs::set_permissions(
        work.join("skills/tdd/scripts/run.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    write(
        &work.join("skills/review/SKILL.md"),
        "---\nname: review\n---\nr\n",
    );
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "one"]);
    let bare = base.join("github/acme/skills.git");
    fs::create_dir_all(bare.parent().unwrap()).unwrap();
    git(
        &base,
        &[
            "clone",
            "-q",
            "--bare",
            work.to_str().unwrap(),
            bare.to_str().unwrap(),
        ],
    );

    let user_home = base.join("home");
    let proj = user_home.join("proj");
    fs::create_dir_all(proj.join(".git")).unwrap();
    write(&proj.join("AGENTS.md"), "# 规则\n");
    write(
        &proj.join(".mcp.json"),
        "{\n  \"mcpServers\": {\n    \"zeta\": { \"command\": \"z\" },\n    \"alpha\": { \"command\": \"a\" }\n  }\n}\n",
    );
    write(
        &proj.join(".codex/config.toml"),
        "# 人写的注释\nmodel = \"o3\"\n\n[mcp_servers.other]\ncommand = \"o\"\n",
    );
    write(&proj.join(".gitignore"), "node_modules/\n");
    let home = FangcunHome::at(user_home.join(".fangcun"));
    write(
        &home.projects_file(),
        &format!(
            "[[project]]\npath = \"{}\"\nregistered = \"2026-11-19T00:00:00Z\"\n",
            proj.display()
        ),
    );
    Fixture {
        base,
        home,
        user_home,
        proj,
    }
}

fn kernel(f: &Fixture) -> Kernel {
    Kernel::with_home(f.home.clone(), Access::ReadWrite)
        .unwrap()
        .with_user_home(&f.user_home)
        .with_project(&f.proj)
        .with_github_base(format!("file://{}", f.base.join("github").display()))
}

fn manifest(mode: &str, skills: &str, mcp: bool) -> String {
    let mut text = format!(
        "[project]\ntargets = [\"claude-code\", \"codex\"]\nmode = \"{mode}\"\n\n[skills]\n{skills}\n"
    );
    if mcp {
        text.push_str(
            "\n[mcp.github]\ncommand = \"npx\"\nargs = [\"-y\", \"server-github\"]\nenv = { LOG = \"info\" }\n\n[mcp.secretive]\ncommand = \"s\"\nenv = { TOKEN = \"secret:secretive-token\" }\n",
        );
    }
    text.push_str("\n[instructions]\ncanonical = \"AGENTS.md\"\nmirror = [\"CLAUDE.md\"]\n");
    text
}

const TDD_DECL: &str = "tdd = { source = \"github:acme/skills\" }";

/// 写清单、写锁、同步，返回同步的计划标题和提示。
fn lock_and_sync(kernel: &Kernel, f: &Fixture, text: &str) -> crate::materializer::ChangePlan {
    write(&f.proj.join("fangcun.toml"), text);
    let lock = kernel.plan_lock(&f.proj).unwrap();
    kernel.apply(&lock).unwrap();
    let plan = kernel.plan_sync(&f.proj).unwrap();
    kernel.apply(&plan).unwrap();
    plan
}

/// 目录树的样子：相对路径 → 内容（软链写成 `-> 目标`），带权限。
fn tree(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let mut entries: Vec<_> = fs::read_dir(dir).unwrap().flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if rel == ".git" {
                continue;
            }
            let meta = fs::symlink_metadata(&path).unwrap();
            if meta.file_type().is_symlink() {
                out.insert(
                    rel,
                    format!("-> {}", fs::read_link(&path).unwrap().display()),
                );
            } else if meta.is_dir() {
                out.insert(rel.clone(), "dir".to_owned());
                walk(root, &path, out);
            } else {
                let mode = meta.permissions().mode() & 0o777;
                out.insert(
                    rel,
                    format!("{mode:o} {}", fs::read_to_string(&path).unwrap_or_default()),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn link_mode_sync_writes_only_its_own_things_and_rolls_back_byte_for_byte() {
    let f = fixture("link");
    let kernel = kernel(&f);
    let before = tree(&f.proj);
    let plan = lock_and_sync(&kernel, &f, &manifest("link", TDD_DECL, true));
    assert!(plan.title.starts_with("同步 "), "{}", plan.title);
    assert!(
        plan.notes
            .iter()
            .any(|n| n.tone == NoteTone::Warning && n.text.contains("secretive")),
        "{:?}",
        plan.notes
    );

    // Skill：实体是指向资产库的软链，Claude Code 的目录是相对软链。
    let entity = fs::read_link(f.proj.join(".agents/skills/tdd")).unwrap();
    assert!(
        entity.starts_with(f.home.library_dir()),
        "{}",
        entity.display()
    );
    assert_eq!(
        fs::read_to_string(f.proj.join(".agents/skills/tdd/SKILL.md")).unwrap(),
        TDD
    );
    assert_eq!(
        fs::read_link(f.proj.join(".claude/skills/tdd")).unwrap(),
        PathBuf::from("../../.agents/skills/tdd")
    );
    // MCP：别人写的键和顺序不动，用了密钥的不写。
    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(f.proj.join(".mcp.json")).unwrap()).unwrap();
    let servers: Vec<&String> = mcp["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(servers, ["zeta", "alpha", "github"]);
    assert_eq!(mcp["mcpServers"]["github"]["args"][1], "server-github");
    let codex = fs::read_to_string(f.proj.join(".codex/config.toml")).unwrap();
    assert!(codex.starts_with("# 人写的注释\nmodel = \"o3\""), "{codex}");
    assert!(codex.contains("[mcp_servers.github]") && codex.contains("[mcp_servers.other]"));
    assert!(!codex.contains("secretive"));
    // 指令镜像、.gitignore 方寸那一段。
    assert_eq!(
        fs::read_to_string(f.proj.join("CLAUDE.md")).unwrap(),
        "@AGENTS.md\n"
    );
    let ignore = fs::read_to_string(f.proj.join(".gitignore")).unwrap();
    assert!(ignore.starts_with("node_modules/\n"));
    for line in [
        "/.agents/skills/tdd",
        "/.claude/skills/tdd",
        "/fangcun.local.toml",
    ] {
        assert!(ignore.lines().any(|l| l == line), "{ignore}");
    }

    // 再同步一次没有变更；--frozen 校验通过。
    let again = kernel.plan_sync(&f.proj).unwrap();
    assert!(again.changes.is_empty(), "{:?}", again.changes);
    kernel.verify_frozen(&f.proj).unwrap();

    // 清单里去掉 tdd 和 github：链接、键都删掉，别人的键还在。
    let after_first = tree(&f.proj);
    let removal = lock_and_sync(&kernel, &f, &manifest("link", "", false));
    assert!(!f.proj.join(".agents/skills/tdd").exists());
    assert!(fs::symlink_metadata(f.proj.join(".claude/skills/tdd")).is_err());
    let mcp = fs::read_to_string(f.proj.join(".mcp.json")).unwrap();
    assert!(mcp.contains("zeta") && !mcp.contains("github"), "{mcp}");
    assert!(
        removal
            .changes
            .iter()
            .any(|c| c.detail.contains("清单里去掉了 tdd"))
    );

    // 回滚这次去掉：回到第一次同步后的样子。
    let records = kernel.log(&RecordFilter::default()).unwrap();
    let last_sync = records
        .iter()
        .find(|r| r.kind == RecordKind::Sync && r.title.starts_with("同步"))
        .unwrap();
    let undo = kernel.plan_rollback_record(&last_sync.id).unwrap();
    kernel.apply(&undo).unwrap();
    assert_eq!(
        tree(&f.proj)
            .into_iter()
            .filter(|(k, _)| !k.starts_with("fangcun."))
            .collect::<BTreeMap<_, _>>(),
        after_first
            .clone()
            .into_iter()
            .filter(|(k, _)| !k.starts_with("fangcun."))
            .collect::<BTreeMap<_, _>>()
    );
    assert_eq!(
        kernel
            .log(&RecordFilter::default())
            .unwrap()
            .iter()
            .find(|r| r.id == last_sync.id)
            .unwrap()
            .status,
        RecordStatus::RolledBack
    );

    // 从新到旧一条条回滚，每条都不该被挡住；最后除了手写的清单，项目和最初逐字节一致。
    for _ in 0..20 {
        let Some(next) = kernel
            .log(&RecordFilter::default())
            .unwrap()
            .into_iter()
            .find(|r| r.status == RecordStatus::Applied && r.kind != RecordKind::Rollback)
        else {
            break;
        };
        let plan = kernel.plan_rollback_record(&next.id).unwrap();
        assert!(!plan.blocked, "{}：{:?}", next.title, plan.notes);
        kernel.apply(&plan).unwrap();
    }
    assert!(!f.proj.join("fangcun.lock").exists());
    let mut now = tree(&f.proj);
    now.retain(|k, _| !k.starts_with("fangcun."));
    assert_eq!(now, before);
}

#[test]
fn vendor_mode_copies_files_and_replaces_an_edited_copy_with_a_backup() {
    let f = fixture("vendor");
    // skills 命令行装过、后来改过的一份。
    write(
        &f.proj.join(".agents/skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\n改过\n",
    );
    let kernel = kernel(&f);
    let before = tree(&f.proj);
    let plan = lock_and_sync(&kernel, &f, &manifest("vendor", TDD_DECL, false));
    assert!(
        plan.notes
            .iter()
            .any(|n| n.tone == NoteTone::Warning && n.text.contains(".agents/skills/tdd")),
        "{:?}",
        plan.notes
    );
    let dir = f.proj.join(".agents/skills/tdd");
    assert!(fs::symlink_metadata(&dir).unwrap().is_dir());
    assert_eq!(fs::read_to_string(dir.join("SKILL.md")).unwrap(), TDD);
    let mode = fs::metadata(dir.join("scripts/run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111, "脚本要保留可执行权限");
    // 入库模式的实体要提交，不进 .gitignore。
    let ignore = fs::read_to_string(f.proj.join(".gitignore")).unwrap();
    assert!(!ignore.contains("/.agents/skills/tdd"), "{ignore}");

    // 回滚：改过的那份原样回来。
    let plan = kernel.plan_rollback(&f.proj, SnapshotId(1)).unwrap();
    kernel.apply(&plan).unwrap();
    let mut now = tree(&f.proj);
    now.retain(|k, _| !k.starts_with("fangcun."));
    assert_eq!(now, before);
}

#[test]
fn switching_link_to_vendor_and_back_leaves_nothing_behind() {
    let f = fixture("switch");
    let kernel = kernel(&f);
    lock_and_sync(&kernel, &f, &manifest("link", TDD_DECL, false));
    let linked = tree(&f.proj);
    lock_and_sync(&kernel, &f, &manifest("vendor", TDD_DECL, false));
    assert!(
        fs::symlink_metadata(f.proj.join(".agents/skills/tdd"))
            .unwrap()
            .is_dir()
    );
    assert!(f.proj.join(".agents/skills/tdd/scripts/run.sh").is_file());
    // 撤销这次切换：实体目录整个换回软链，不留文件。
    let records = kernel.log(&RecordFilter::default()).unwrap();
    let switch = records
        .iter()
        .find(|r| r.title.starts_with("同步"))
        .unwrap();
    let plan = kernel.plan_rollback_record(&switch.id).unwrap();
    assert!(!plan.blocked, "{:?}", plan.notes);
    kernel.apply(&plan).unwrap();
    let mut now = tree(&f.proj);
    let mut linked = linked;
    now.retain(|k, _| !k.starts_with("fangcun."));
    linked.retain(|k, _| !k.starts_with("fangcun."));
    assert_eq!(now, linked);
}

#[test]
fn rolling_back_an_older_write_waits_for_the_later_one() {
    let f = fixture("conflict");
    let kernel = kernel(&f);
    lock_and_sync(&kernel, &f, &manifest("link", TDD_DECL, false));
    let first = kernel
        .log(&RecordFilter::default())
        .unwrap()
        .into_iter()
        .find(|r| r.title.starts_with("同步"))
        .unwrap();
    // 后来又一次同步改了 .gitignore（加了 review）。
    lock_and_sync(
        &kernel,
        &f,
        &manifest(
            "link",
            &format!("{TDD_DECL}\nreview = {{ source = \"github:acme/skills\" }}"),
            false,
        ),
    );
    let plan = kernel.plan_rollback_record(&first.id).unwrap();
    assert!(plan.blocked);
    assert!(
        plan.changes
            .iter()
            .any(|c| c.detail.contains("之后也改过这个文件"))
    );
    let note = &plan
        .notes
        .iter()
        .find(|n| n.tone == NoteTone::Warning)
        .unwrap()
        .text;
    assert!(note.contains("先回滚「同步"), "{note}");
    assert!(matches!(kernel.apply(&plan), Err(Error::Blocked(_))));
}

#[test]
fn frozen_check_fails_and_is_recorded_when_the_lock_is_stale() {
    let f = fixture("frozen");
    let kernel = kernel(&f);
    lock_and_sync(&kernel, &f, &manifest("link", TDD_DECL, false));
    write(
        &f.proj.join("fangcun.toml"),
        &manifest(
            "link",
            &format!("{TDD_DECL}\nreview = {{ source = \"github:acme/skills\" }}"),
            false,
        ),
    );
    assert!(matches!(
        kernel.verify_frozen(&f.proj),
        Err(Error::Frozen(_))
    ));
    assert!(matches!(kernel.plan_sync(&f.proj), Err(Error::Frozen(_))));
    let verify = kernel
        .log(&RecordFilter::default())
        .unwrap()
        .into_iter()
        .find(|r| r.kind == RecordKind::Verify)
        .unwrap();
    assert_eq!(verify.status, RecordStatus::Failed);
    assert!(verify.summary.contains("review"));
}

#[test]
fn expired_snapshots_are_pruned_and_say_so() {
    let f = fixture("prune");
    write(
        &f.home.settings_file(),
        "snapshot-days = 0\nsnapshot-keep = 1\n",
    );
    let kernel = kernel(&f);
    lock_and_sync(&kernel, &f, &manifest("link", TDD_DECL, false));
    let records = kernel.log(&RecordFilter::default()).unwrap();
    let oldest = records.last().unwrap();
    match kernel.plan_rollback_record(&oldest.id) {
        Err(Error::Blocked(why)) => assert!(why.contains("已过期"), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn adopt_merges_copies_into_the_library_and_rolls_back() {
    let f = fixture("adopt");
    // 全局层两份相同（Codex 读 ~/.agents/skills，Claude Code 的是软链），项目里一份分叉的，
    // 一份在未登记的项目里，一份软链到本地 skill 仓库。
    let global = f.user_home.join(".agents/skills/harness");
    write(&global.join("SKILL.md"), "---\nname: harness\n---\nsame\n");
    fs::create_dir_all(f.user_home.join(".claude/skills")).unwrap();
    write(
        &f.user_home.join(".claude/skills/harness/SKILL.md"),
        "---\nname: harness\n---\nsame\n",
    );
    write(
        &f.proj.join(".claude/skills/harness/SKILL.md"),
        "---\nname: harness\n---\nforked\n",
    );
    let other = f.user_home.join("other");
    fs::create_dir_all(other.join(".git")).unwrap();
    write(
        &other.join(".claude/skills/harness/SKILL.md"),
        "---\nname: harness\n---\nsame\n",
    );
    let repo = f.user_home.join("dev/skills/harness");
    write(&repo.join("SKILL.md"), "---\nname: harness\n---\nsame\n");
    fs::create_dir_all(f.proj.join(".agents/skills")).unwrap();
    symlink(&repo, f.proj.join(".agents/skills/harness")).unwrap();

    let kernel = Kernel::with_home(f.home.clone(), Access::ReadWrite)
        .unwrap()
        .with_user_home(&f.user_home)
        .with_project(&f.proj)
        .with_project(&other);
    let before_home = tree(&f.user_home.join(".claude"));
    let before_proj = tree(&f.proj);
    let plan = kernel.plan_adopt("harness").unwrap();
    assert_eq!(plan.title, "收编 harness：3 份副本归并为 2 份正本");
    let texts: Vec<&str> = plan.notes.iter().map(|n| n.text.as_str()).collect();
    assert!(texts.iter().any(|t| t.contains("已分叉")), "{texts:?}");
    assert!(
        texts
            .iter()
            .any(|t| t.contains("不在写入范围") && t.contains("other")),
        "{texts:?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("1 份是指向本地目录的软链")
                && t.contains("proj/.agents/skills/harness")),
        "{texts:?}"
    );
    assert!(
        texts.iter().any(|t| t.starts_with("1 份不在写入范围")),
        "{texts:?}"
    );
    kernel.apply(&plan).unwrap();

    let library = f.home.library_dir();
    for place in [
        global.clone(),
        f.user_home.join(".claude/skills/harness"),
        f.proj.join(".claude/skills/harness"),
    ] {
        let target =
            fs::read_link(&place).unwrap_or_else(|_| panic!("{} 应是软链", place.display()));
        assert!(target.starts_with(&library), "{}", target.display());
    }
    assert_eq!(
        fs::read_to_string(f.proj.join(".claude/skills/harness/SKILL.md")).unwrap(),
        "---\nname: harness\n---\nforked\n",
        "分叉的那份内容不变"
    );
    assert_eq!(
        fs::read_link(f.proj.join(".agents/skills/harness")).unwrap(),
        repo
    );
    assert!(other.join(".claude/skills/harness").is_dir());
    let adopted = fs::read_to_string(f.home.state_dir().join("adopted.toml")).unwrap();
    assert_eq!(adopted.matches("[[link]]").count(), 3);

    // 回滚：全局层和项目里的副本原样回来。
    let record = kernel
        .log(&RecordFilter::default())
        .unwrap()
        .into_iter()
        .find(|r| r.kind == RecordKind::Adopt)
        .unwrap();
    assert!(
        record.snapshots.len() >= 2,
        "跨范围各留一个快照：{:?}",
        record.snapshots
    );
    let undo = kernel.plan_rollback_record(&record.id).unwrap();
    kernel.apply(&undo).unwrap();
    assert_eq!(tree(&f.user_home.join(".claude")), before_home);
    assert_eq!(tree(&f.proj), before_proj);
    assert!(fs::symlink_metadata(&global).unwrap().is_dir());
}

/// 按项目停用（M1-18）：全局装了 lark-doc、lark-im、keep-me 三个 skill 和 playwright 一个 MCP，
/// 清单 `[disable]` 停用 `lark-*` 和 playwright。同步后写进三处开关，生效上下文显示「已停用」；
/// 从清单里删掉后再同步，只撤方寸写的，别人写的键原样留着。
#[test]
fn disable_writes_each_tool_switch_and_takes_back_only_its_own() {
    let f = fixture("disable");
    for name in ["lark-doc", "lark-im", "keep-me"] {
        let text = format!("---\nname: {name}\ndescription: d\n---\nx\n");
        write(
            &f.user_home.join(format!(".claude/skills/{name}/SKILL.md")),
            &text,
        );
        write(
            &f.user_home.join(format!(".agents/skills/{name}/SKILL.md")),
            &text,
        );
    }
    let proj = f.proj.display().to_string();
    write(
        &f.user_home.join(".claude.json"),
        &format!(
            "{{\n  \"mcpServers\": {{ \"playwright\": {{ \"command\": \"p\" }} }},\n  \"projects\": {{ \"{proj}\": {{ \"hasTrustDialogAccepted\": true }} }}\n}}\n"
        ),
    );
    write(
        &f.user_home.join(".codex/config.toml"),
        &format!(
            "[mcp_servers.playwright]\ncommand = \"p\"\n\n[projects.\"{proj}\"]\ntrust_level = \"trusted\"\n"
        ),
    );
    write(
        &f.proj.join(".claude/settings.json"),
        "{\n  \"skillOverrides\": { \"mine\": \"name-only\" }\n}\n",
    );
    let k = kernel(&f);
    let base = manifest("link", TDD_DECL, false);
    let with = format!("{base}\n[disable]\nskills = [\"lark-*\"]\nmcp = [\"playwright\"]\n");
    let plan = lock_and_sync(&k, &f, &with);
    assert!(
        plan.notes.iter().all(|n| n.tone != NoteTone::Warning),
        "受信任的项目不该有提醒：{:?}",
        plan.notes
    );

    let settings = fs::read_to_string(f.proj.join(".claude/settings.json")).unwrap();
    assert!(
        settings.contains("\"lark-doc\": \"off\"") && settings.contains("\"lark-im\": \"off\"")
    );
    assert!(!settings.contains("keep-me") && settings.contains("\"mine\": \"name-only\""));
    let codex = fs::read_to_string(f.proj.join(".codex/config.toml")).unwrap();
    assert!(
        codex.starts_with("# 人写的注释"),
        "别人写的注释要留着：{codex}"
    );
    assert!(codex.contains("[[skills.config]]\nname = \"lark-doc\"\nenabled = false"));
    assert!(codex.contains("[mcp_servers.playwright]\nenabled = false"));
    let claude_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(f.user_home.join(".claude.json")).unwrap())
            .unwrap();
    assert_eq!(
        claude_json["projects"][proj.as_str()]["disabledMcpServers"],
        serde_json::json!(["playwright"])
    );
    assert_eq!(
        claude_json["projects"][proj.as_str()]["hasTrustDialogAccepted"],
        true
    );

    // 换一个内核重新扫描：两个工具都显示已停用，不计入常驻。
    let fresh = kernel(&f);
    for tool in ["claude-code", "codex"] {
        let context = fresh
            .effective_context(&f.proj, &crate::model::ToolId::from(tool))
            .unwrap();
        for name in ["lark-doc", "playwright"] {
            let item = context
                .items
                .iter()
                .find(|i| i.name == name && i.label == "已停用")
                .unwrap_or_else(|| panic!("{tool} 下 {name} 应显示已停用"));
            assert!(!item.counted);
            let option = item.disable.as_ref().expect("全局层的项能在本项目停用");
            assert!(option.active && option.unsupported.is_none());
        }
        assert!(
            context
                .items
                .iter()
                .any(|i| i.name == "keep-me" && i.label != "已停用"),
            "{tool} 下 keep-me 不该停用"
        );
    }

    // 界面暂存的「在本项目停用」：清单和开关一份计划写。
    let k = kernel(&f);
    let plan = k
        .plan_sync_with(
            &f.proj,
            &[crate::manifest::ManifestEdit::Disable {
                kind: crate::model::AssetKind::Skill,
                names: vec!["keep-me".to_owned()],
            }],
        )
        .unwrap();
    assert!(
        plan.changes
            .iter()
            .any(|c| c.path.ends_with("fangcun.toml"))
    );
    k.apply(&plan).unwrap();
    let manifest_text = fs::read_to_string(f.proj.join("fangcun.toml")).unwrap();
    assert!(
        manifest_text.starts_with(&base),
        "清单原文不动：{manifest_text}"
    );
    assert!(manifest_text.contains("\"keep-me\""));
    let settings = fs::read_to_string(f.proj.join(".claude/settings.json")).unwrap();
    assert!(settings.contains("\"keep-me\": \"off\""));

    // 从清单里删掉：撤回方寸写的，别人写的留着。
    lock_and_sync(&kernel(&f), &f, &base);
    let settings = fs::read_to_string(f.proj.join(".claude/settings.json")).unwrap();
    assert!(!settings.contains("lark-") && settings.contains("\"mine\": \"name-only\""));
    let codex = fs::read_to_string(f.proj.join(".codex/config.toml")).unwrap();
    assert!(
        !codex.contains("skills.config") && !codex.contains("playwright"),
        "{codex}"
    );
    assert!(codex.starts_with("# 人写的注释"));
    let claude_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(f.user_home.join(".claude.json")).unwrap())
            .unwrap();
    assert_eq!(
        claude_json["projects"][proj.as_str()]["disabledMcpServers"],
        serde_json::json!([])
    );
}

/// 目录下全部文件的文本，找明文用。
fn all_text(dir: &Path) -> String {
    let mut out = String::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.push_str(&all_text(&path));
        } else {
            out.push_str(&String::from_utf8_lossy(
                &fs::read(&path).unwrap_or_default(),
            ));
        }
    }
    out
}

#[test]
fn secrets_go_to_the_personal_layer_and_never_into_plans_snapshots_or_the_journal() {
    const TOKEN: &str = "tok_abcdefghijklmnop";
    let f = fixture("secrets");
    let store = std::sync::Arc::new(crate::secrets::MemoryStore::with(&[(
        "secretive-token",
        TOKEN,
    )]));
    let kernel = kernel(&f).with_secret_store(store.clone());
    let claude_json = f.user_home.join(".claude.json");
    write(&claude_json, "{\n  \"numStartups\": 3\n}\n");
    let project = f.proj.to_string_lossy().into_owned();

    let first = lock_and_sync(&kernel, &f, &manifest("link", TDD_DECL, true));
    // 提交的配置只写环境变量引用：Claude Code 写 ${TOKEN}，Codex 写 env_vars。
    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(f.proj.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["secretive"]["env"]["TOKEN"], "${TOKEN}");
    let codex = fs::read_to_string(f.proj.join(".codex/config.toml")).unwrap();
    assert!(codex.contains("[mcp_servers.secretive]"), "{codex}");
    assert!(codex.contains("env_vars = [\"TOKEN\"]"), "{codex}");
    assert!(!codex.contains(TOKEN));
    assert!(
        first
            .notes
            .iter()
            .any(|n| n.text.contains("Codex 从启动它的环境里读 TOKEN")),
        "{:?}",
        first.notes
    );
    // 明文只在 Claude Code 的个人层：~/.claude.json 里这个项目那一节，别的键不动。
    let personal: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert_eq!(personal["numStartups"], 3);
    assert_eq!(
        personal["projects"][&project]["mcpServers"]["secretive"]["env"]["TOKEN"],
        TOKEN
    );
    assert!(personal["projects"][&project]["mcpServers"]["github"].is_null());
    assert!(!serde_json::to_string(&first).unwrap().contains(TOKEN));

    // 项目页：个人层那条显示「密钥在钥匙串」，项目层的引用并进它，不单列；写入与活动从台账、记录来。
    kernel.rescan().unwrap();
    let context = kernel
        .effective_context(
            &f.proj,
            &crate::model::ToolId::from(crate::model::ToolId::CLAUDE_CODE),
        )
        .unwrap();
    let rows: Vec<_> = context
        .items
        .iter()
        .filter(|i| i.name == "secretive")
        .collect();
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = rows[0];
    assert_eq!(row.label, "密钥在钥匙串");
    assert_eq!(row.secrets, ["TOKEN → 钥匙串 secretive-token"]);
    assert!(
        row.files
            .iter()
            .any(|l| l == ".mcp.json → mcpServers.secretive（按键合并）"),
        "{:?}",
        row.files
    );
    assert!(
        row.files.iter().any(|l| l.contains("本机个人层")),
        "{:?}",
        row.files
    );
    assert!(!row.recent.is_empty());
    assert!(!serde_json::to_string(&context).unwrap().contains(TOKEN));
    let tdd = context.items.iter().find(|i| i.name == "tdd").unwrap();
    assert!(
        tdd.files.iter().any(|l| l == ".agents/skills/tdd → 资产库"),
        "{:?}",
        tdd.files
    );
    assert!(
        tdd.recent.iter().any(|a| a.text.contains("tdd")),
        "{:?}",
        tdd.recent
    );

    // 清单页签：MCP 的密钥只给名称和在不在钥匙串里。
    let view = kernel.manifest_view(&f.proj).unwrap().unwrap();
    let names: Vec<&str> = view.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        ["fangcun.toml", "fangcun.lock", "fangcun.local.toml"]
    );
    let toml = &view.files[0];
    let mcp = toml
        .sections
        .iter()
        .find(|s| s.title == "MCP 服务")
        .unwrap();
    let secretive = mcp.rows.iter().find(|r| r.cells[0] == "secretive").unwrap();
    assert_eq!(secretive.cells[2], "TOKEN 存在钥匙串");
    assert!(!view.files[2].exists);
    assert!(
        view.files[1].sections[0]
            .rows
            .iter()
            .any(|r| r.cells[0] == "tdd")
    );
    assert!(
        toml.lines
            .iter()
            .any(|l| l.contains("secret:secretive-token"))
    );

    // 个人覆盖加一个服务：写进个人层；Codex 没有个人层，写一条说明。
    let before_second = fs::read(&claude_json).unwrap();
    write(
        &f.proj.join("fangcun.local.toml"),
        "[mcp.mine]\ncommand = \"m\"\n",
    );
    let second = kernel.plan_sync(&f.proj).unwrap();
    assert!(
        second
            .notes
            .iter()
            .any(|n| n.text.contains("个人覆盖里的 MCP 对它不生效：mine")),
        "{:?}",
        second.notes
    );
    let applied = kernel.apply(&second).unwrap();
    let personal: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert_eq!(
        personal["projects"][&project]["mcpServers"]["mine"]["command"],
        "m"
    );
    assert!(
        !fs::read_to_string(f.proj.join(".mcp.json"))
            .unwrap()
            .contains("mine")
    );

    // 快照里存的 ~/.claude.json 换掉了明文，活动记录里也没有。
    assert!(!all_text(&f.home.snapshots_dir()).contains(TOKEN));
    assert!(!all_text(&f.home.journal_dir()).contains(TOKEN));

    // 回滚第二次同步：明文从钥匙串取回，逐字节一致。
    let undo = kernel.plan_rollback_record(&applied.record).unwrap();
    assert!(!undo.blocked, "{:?}", undo.notes);
    kernel.apply(&undo).unwrap();
    assert_eq!(fs::read(&claude_json).unwrap(), before_second);

    // 钥匙串里删掉之后，再回滚到那份快照就挡住了。
    let again = kernel.plan_sync(&f.proj).unwrap();
    let applied = kernel.apply(&again).unwrap();
    use crate::secrets::SecretStore as _;
    store.delete("secretive-token").unwrap();
    let undo = kernel.plan_rollback_record(&applied.record).unwrap();
    assert!(undo.blocked);
    assert!(
        undo.notes
            .iter()
            .any(|n| n.text.contains("钥匙串里没有 secretive-token")),
        "{:?}",
        undo.notes
    );
}

// ───────────── 漂移与更新（M1-15、M1-16） ─────────────

/// 在上游仓库提交一版新的 tdd，推到冒充 GitHub 的裸仓库。返回新提交。
fn push_new_tdd(f: &Fixture, text: &str, script: bool) -> String {
    let work = f.base.join("work");
    write(&work.join("skills/tdd/SKILL.md"), text);
    if script {
        write(&work.join("skills/tdd/scripts/new.sh"), "rm -rf /tmp/x\n");
    }
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "two"]);
    let bare = f.base.join("github/acme/skills.git");
    git(&work, &["push", "-q", bare.to_str().unwrap(), "main"]);
    git(&work, &["rev-parse", "HEAD"])
}

fn claude() -> crate::model::ToolId {
    crate::model::ToolId::from(crate::model::ToolId::CLAUDE_CODE)
}

#[test]
fn upstream_check_finds_a_new_version_and_upgrade_settles_the_pending_record() {
    let f = fixture("upgrade");
    let kernel = kernel(&f);
    lock_and_sync(&kernel, &f, &manifest("link", TDD_DECL, false));
    assert!(kernel.check_upstream().unwrap().items.is_empty());

    let new = push_new_tdd(&f, "---\nname: tdd\n---\nv2\n", true);
    let report = kernel.check_upstream().unwrap();
    assert_eq!(report.items.len(), 1, "{report:?}");
    let item = &report.items[0];
    assert_eq!(item.to, &new[..7]);
    assert!(item.scripts, "{item:?}");
    // 待处理一条，再查一次不重复记。
    kernel.check_upstream().unwrap();
    let pending = |k: &Kernel| {
        k.log(&RecordFilter::default())
            .unwrap()
            .into_iter()
            .filter(|r| r.kind == RecordKind::Updatable && r.status == RecordStatus::Pending)
            .count()
    };
    assert_eq!(pending(&kernel), 1);
    assert_eq!(kernel.status(&f.proj).unwrap().updatable.len(), 1);

    kernel.rescan().unwrap();
    let context = kernel.effective_context(&f.proj, &claude()).unwrap();
    let tdd = context.items.iter().find(|i| i.name == "tdd").unwrap();
    assert_eq!(tdd.label, format!("可更新 ↑ {}", &new[..7]));
    let update = tdd.update.as_ref().unwrap();
    assert!(update.scripts && update.files.iter().any(|f| f.path == "scripts/new.sh"));

    // 从同步面板暂存升级：锁换到新版，软链指向新正本。
    let plan = kernel
        .plan_sync_with(
            &f.proj,
            &[crate::manifest::ManifestEdit::Upgrade {
                asset: "tdd".to_owned(),
                projects: Vec::new(),
            }],
        )
        .unwrap();
    kernel.apply(&plan).unwrap();
    let lock = crate::manifest::Lock::load(&f.proj).unwrap().unwrap();
    assert_eq!(lock.skills["tdd"].commit.as_deref(), Some(new.as_str()));
    assert_eq!(
        fs::read_to_string(f.proj.join(".agents/skills/tdd/SKILL.md")).unwrap(),
        "---\nname: tdd\n---\nv2\n"
    );
    assert_eq!(pending(&kernel), 0);
    assert!(kernel.status(&f.proj).unwrap().updatable.is_empty());
}

#[test]
fn link_mode_drift_is_reported_discarded_and_recoverable_by_rollback() {
    let f = fixture("drift-link");
    let kernel = kernel(&f);
    lock_and_sync(&kernel, &f, &manifest("link", TDD_DECL, false));
    assert!(kernel.drifts(&f.proj).unwrap().is_empty());

    // Agent 经 Claude Code 的目录改 skill：改到的是资产库正本。
    let skill = f.proj.join(".claude/skills/tdd/SKILL.md");
    fs::write(&skill, "---\nname: tdd\n---\nv1\n多一行\n").unwrap();
    let found = kernel.drifts(&f.proj).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].object.is_some());

    // 活动页：一条待处理，不重复记。
    let pending = |k: &Kernel| {
        k.log(&RecordFilter::default())
            .unwrap()
            .into_iter()
            .filter(|r| r.kind == RecordKind::Drift && r.status == RecordStatus::Pending)
            .count()
    };
    assert_eq!(pending(&kernel), 1);
    assert_eq!(pending(&kernel), 1);

    kernel.rescan().unwrap();
    let context = kernel.effective_context(&f.proj, &claude()).unwrap();
    let tdd = context.items.iter().find(|i| i.name == "tdd").unwrap();
    assert_eq!(tdd.label, "已漂移");
    let drift = tdd.drift.as_ref().unwrap();
    assert!(drift.original && drift.shared);
    assert!(drift.summary.contains("多 1 行"), "{}", drift.summary);
    assert!(drift.promote_blocked.is_some());
    assert!(
        drift.files[0]
            .lines
            .iter()
            .any(|l| l.kind == crate::library::diff::DiffKind::Add && l.text == "多一行")
    );

    // 丢弃：正本从来源取回原版。
    let plan = kernel
        .plan_drift(
            &f.proj,
            "tdd",
            crate::materializer::drift::DriftChoice::Discard,
        )
        .unwrap();
    let applied = kernel.apply(&plan).unwrap();
    assert_eq!(fs::read_to_string(&skill).unwrap(), TDD);
    assert!(kernel.drifts(&f.proj).unwrap().is_empty());
    assert_eq!(pending(&kernel), 0);

    // 丢弃的修改能靠回滚找回（6.7 回滚规则第 4 条）。
    let undo = kernel.plan_rollback_record(&applied.record).unwrap();
    kernel.apply(&undo).unwrap();
    assert!(fs::read_to_string(&skill).unwrap().contains("多一行"));
}

#[test]
fn fork_turns_a_vendored_copy_into_a_project_file() {
    let f = fixture("drift-fork");
    let kernel = kernel(&f);
    lock_and_sync(&kernel, &f, &manifest("vendor", TDD_DECL, false));
    let dir = f.proj.join(".agents/skills/tdd");
    fs::write(dir.join("SKILL.md"), "改过的\n").unwrap();
    let drift = kernel.drifts(&f.proj).unwrap();
    assert_eq!(drift.len(), 1);
    assert!(drift[0].object.is_none());

    let plan = kernel
        .plan_drift(
            &f.proj,
            "tdd",
            crate::materializer::drift::DriftChoice::Fork,
        )
        .unwrap();
    kernel.apply(&plan).unwrap();
    assert_eq!(
        fs::read_to_string(dir.join("SKILL.md")).unwrap(),
        "改过的\n"
    );
    let manifest = fs::read_to_string(f.proj.join("fangcun.toml")).unwrap();
    assert!(!manifest.contains("tdd ="), "{manifest}");
    let lock = crate::manifest::Lock::load(&f.proj).unwrap().unwrap();
    assert!(!lock.skills.contains_key("tdd"));
    assert!(kernel.drifts(&f.proj).unwrap().is_empty());
    // 之后同步不再碰它。
    let again = kernel.plan_sync(&f.proj).unwrap();
    assert!(
        again.changes.iter().all(|c| !c.path.starts_with(&dir)),
        "{:?}",
        again.changes
    );
}

#[test]
fn promote_commits_to_the_local_source_and_repins_the_project() {
    let f = fixture("drift-promote");
    // 本地来源：一个有 git 历史的 skill 仓库。
    let mine = f.base.join("mine");
    write(
        &mine.join("harness/SKILL.md"),
        "---\nname: harness\n---\nh1\n",
    );
    git(&mine, &["init", "-q", "-b", "main"]);
    // 回流的提交用来源仓库的 git 身份（CI 上没有全局的）。
    git(&mine, &["config", "user.name", "t"]);
    git(&mine, &["config", "user.email", "t@example.com"]);
    git(&mine, &["add", "-A"]);
    git(&mine, &["commit", "-q", "-m", "h1"]);
    let first = git(&mine, &["rev-parse", "HEAD"]);
    crate::library::source::Sources::open(&f.home)
        .remember_local("mine", &mine)
        .unwrap();
    let kernel = kernel(&f);
    let decl = format!(
        "harness = {{ source = \"local:mine\", pin = \"{}\" }}",
        &first[..7]
    );
    lock_and_sync(&kernel, &f, &manifest("link", &decl, false));

    fs::write(
        f.proj.join(".claude/skills/harness/SKILL.md"),
        "---\nname: harness\n---\nh2\n",
    )
    .unwrap();
    kernel.rescan().unwrap();
    let context = kernel.effective_context(&f.proj, &claude()).unwrap();
    let item = context.items.iter().find(|i| i.name == "harness").unwrap();
    assert!(item.drift.as_ref().unwrap().promote_blocked.is_none());

    let plan = kernel
        .plan_drift(
            &f.proj,
            "harness",
            crate::materializer::drift::DriftChoice::Promote,
        )
        .unwrap();
    assert!(!plan.blocked, "{:?}", plan.notes);
    // 出计划时分支还没动。
    assert_eq!(git(&mine, &["rev-parse", "HEAD"]), first);
    kernel.apply(&plan).unwrap();

    let head = git(&mine, &["rev-parse", "HEAD"]);
    assert_ne!(head, first);
    assert_eq!(
        fs::read_to_string(mine.join("harness/SKILL.md")).unwrap(),
        "---\nname: harness\n---\nh2\n"
    );
    assert_eq!(git(&mine, &["status", "--porcelain"]), "");
    let manifest = fs::read_to_string(f.proj.join("fangcun.toml")).unwrap();
    assert!(
        manifest.contains(&format!("pin = \"{}\"", &head[..7])),
        "{manifest}"
    );
    let lock = crate::manifest::Lock::load(&f.proj).unwrap().unwrap();
    assert_eq!(
        lock.skills["harness"].commit.as_deref(),
        Some(head.as_str())
    );
    assert!(kernel.drifts(&f.proj).unwrap().is_empty());
    // 项目里看到的是新版，锁定 h1 的旧正本还原了。
    assert_eq!(
        fs::read_to_string(f.proj.join(".claude/skills/harness/SKILL.md")).unwrap(),
        "---\nname: harness\n---\nh2\n"
    );
}

// 批 6：MCP 入库、从全局移除、fangcun add、体检的处理进度。
mod batch6;
