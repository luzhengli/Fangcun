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
