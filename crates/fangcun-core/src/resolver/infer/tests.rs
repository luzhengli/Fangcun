//! 登记与反推的测试：临时目录里造一个「装过 skill 的项目」和冒充 GitHub 的 git 仓库，
//! 经 `Kernel::plan_init` 一直走到写出清单、锁、登记表。

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::kernel::{Access, Kernel};
use crate::library::content::legacy_hash_dir;
use crate::manifest::{Lock, load_project};
use crate::materializer::ChangeOp;
use crate::model::{NoteTone, SyncMode};
use crate::paths::FangcunHome;

struct Fixture {
    base: PathBuf,
    home: FangcunHome,
    user_home: PathBuf,
    proj: PathBuf,
    first: String,
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

/// 上游 acme/skills 有两版 tdd，项目里装的是第一版（`skills` 命令行装的，v1 锁）。
/// 另有一个软链到本机 skill 仓库的 `mine`、一个来源未知的 `orphan`、带明文密钥的 MCP、
/// `CLAUDE.md` 是对 `AGENTS.md` 的引用。
fn fixture(name: &str) -> Fixture {
    let base = std::env::temp_dir().join(format!("fangcun-infer-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let work = base.join("work");
    fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    write(
        &work.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\nv1\n",
    );
    write(&work.join("skills/tdd/scripts/run.sh"), "echo 1\n");
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "one"]);
    let first = git(&work, &["rev-parse", "HEAD"]);
    write(
        &work.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\nv2\n",
    );
    git(&work, &["commit", "-q", "-am", "two"]);
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
    let dev = user_home.join("dev/my-skills");
    fs::create_dir_all(dev.join(".git")).unwrap();
    write(
        &dev.join("skills/mine/SKILL.md"),
        "---\nname: mine\n---\nmine\n",
    );

    let proj = user_home.join("proj");
    fs::create_dir_all(proj.join(".git")).unwrap();
    let installed = proj.join(".agents/skills/tdd");
    write(&installed.join("SKILL.md"), "---\nname: tdd\n---\nv1\n");
    write(&installed.join("scripts/run.sh"), "echo 1\n");
    let v1 = legacy_hash_dir(&installed).0;
    write(
        &proj.join("skills-lock.json"),
        &format!(
            r#"{{ "version": 1, "skills": {{ "tdd": {{ "source": "acme/skills", "sourceType": "github", "computedHash": "{v1}" }} }} }}"#
        ),
    );
    fs::create_dir_all(proj.join(".claude/skills")).unwrap();
    symlink("../../.agents/skills/tdd", proj.join(".claude/skills/tdd")).unwrap();
    symlink(dev.join("skills/mine"), proj.join(".claude/skills/mine")).unwrap();
    write(
        &proj.join(".claude/skills/orphan/SKILL.md"),
        "---\nname: orphan\n---\n?\n",
    );
    write(
        &proj.join(".mcp.json"),
        r#"{ "mcpServers": {
            "github": { "command": "npx", "args": ["-y", "server-github"],
                        "env": { "GITHUB_TOKEN": "ghp_abcdefghijklmnop1234", "LOG": "info" } },
            "remote": { "url": "https://example.com/mcp" }
        } }"#,
    );
    write(&proj.join("AGENTS.md"), "# 规则\n");
    write(&proj.join("CLAUDE.md"), "@AGENTS.md\n");

    Fixture {
        home: FangcunHome::at(user_home.join(".fangcun")),
        base,
        user_home,
        proj,
        first,
    }
}

fn kernel(f: &Fixture, access: Access) -> Kernel {
    Kernel::with_home(f.home.clone(), access)
        .unwrap()
        .with_user_home(&f.user_home)
        .with_project(&f.proj)
        .with_github_base(format!("file://{}", f.base.join("github").display()))
}

#[test]
fn init_from_existing_writes_manifest_lock_and_registry() {
    let f = fixture("full");
    let store = std::sync::Arc::new(crate::secrets::MemoryStore::default());
    let kernel = kernel(&f, Access::ReadWrite).with_secret_store(store.clone());
    let plan = kernel.plan_init(&f.proj, true, None).unwrap();

    assert_eq!(plan.title, "登记 proj");
    let paths: Vec<_> = plan.changes.iter().map(|c| c.path.clone()).collect();
    assert_eq!(
        paths,
        [
            f.proj.join("fangcun.toml"),
            f.proj.join("fangcun.lock"),
            f.home.projects_file(),
            std::path::PathBuf::from("钥匙串 · fangcun / github-token"),
        ]
    );
    assert!(plan.changes.iter().all(|c| c.op == ChangeOp::Add));
    let note = |needle: &str| {
        plan.notes
            .iter()
            .find(|n| n.text.contains(needle))
            .unwrap_or_else(|| panic!("缺少提示「{needle}」：{:?}", plan.notes))
    };
    assert_eq!(note("来源未知").tone, NoteTone::Info);
    assert!(note("来源未知").text.contains("orphan"));
    assert_eq!(note("明文密钥").tone, NoteTone::Danger);
    assert!(note("明文密钥").text.contains("github 的 GITHUB_TOKEN"));
    assert!(note("远程服务").text.contains("remote"));
    // 密钥的值不出内核：提示里没有，计划里也没有。
    assert!(!format!("{plan:?}").contains("ghp_"));

    kernel.apply(&plan).unwrap();

    // 明文迁进钥匙串，记一条密钥记录，记录里没有值（9.3）。
    use crate::secrets::SecretStore as _;
    assert_eq!(
        store.get("github-token").unwrap().unwrap().expose(),
        "ghp_abcdefghijklmnop1234"
    );
    let records = kernel
        .log(&crate::journal::RecordFilter::default())
        .unwrap();
    let secret = records
        .iter()
        .find(|r| r.kind == crate::journal::RecordKind::Secret)
        .unwrap();
    assert_eq!(secret.title, "github-token 迁入钥匙串");
    assert!(!serde_json::to_string(&records).unwrap().contains("ghp_"));

    let project = load_project(&f.proj, &f.home).unwrap().unwrap();
    let manifest = &project.manifest;
    let targets: Vec<&str> = manifest
        .project
        .targets
        .iter()
        .map(|t| t.as_str())
        .collect();
    assert_eq!(targets, ["claude-code", "codex"]);
    let names: Vec<&str> = manifest.skills.keys().map(String::as_str).collect();
    assert_eq!(names, ["mine", "tdd"]);
    let tdd = &manifest.skills["tdd"];
    assert_eq!(tdd.source, "github:acme/skills");
    assert_eq!(
        (
            tdd.pin.as_deref(),
            tdd.track.as_deref(),
            tdd.path.as_deref()
        ),
        (None, None, None)
    );
    assert_eq!(manifest.skills["mine"].source, "local:my-skills");
    let github = &manifest.mcp["github"];
    assert_eq!(github.env["GITHUB_TOKEN"], "secret:github-token");
    assert_eq!(github.env["LOG"], "info");
    assert!(!manifest.mcp.contains_key("remote"));
    let instructions = manifest.instructions.as_ref().unwrap();
    assert_eq!(instructions.canonical, "AGENTS.md");
    assert_eq!(instructions.mirror, ["CLAUDE.md"]);
    let manifest_text = fs::read_to_string(f.proj.join("fangcun.toml")).unwrap();
    assert!(!manifest_text.contains("ghp_"));

    // 锁在和本机那份内容一样的提交（第一版），不是最新版。
    let lock = Lock::load(&f.proj).unwrap().unwrap();
    assert_eq!(lock.skills["tdd"].commit.as_deref(), Some(f.first.as_str()));
    assert_eq!(lock.skills["tdd"].path, "skills/tdd");
    assert_eq!(lock.skills["mine"].path, "skills/mine");
    assert!(lock.instructions.contains_key("AGENTS.md"));

    let registry = crate::registry::load(&f.home).unwrap();
    assert!(crate::registry::contains(&registry, &f.proj));

    // 再登记一次没有变更；锁也已是最新。
    let again = kernel.plan_init(&f.proj, true, None).unwrap();
    assert_eq!(again.title, "proj 已经登记过");
    assert!(again.changes.is_empty());
    assert!(kernel.plan_lock(&f.proj).unwrap().changes.is_empty());
    assert!(
        kernel
            .projects()
            .unwrap()
            .iter()
            .any(|p| p.name == "proj" && p.registered)
    );

    // 导出回 skills-lock.json：tdd 换成新版的写法，本地来源的 mine 导不了。
    let export = kernel.plan_export_skills_lock(&f.proj).unwrap();
    assert_eq!(export.changes.len(), 1);
    assert!(export.notes.iter().any(|n| n.text.contains("mine")));
    kernel.apply(&export).unwrap();
    let exported = crate::manifest::read_skills_lock(
        &f.proj.join("skills-lock.json"),
        &f.proj.join(".agents/skills"),
    )
    .unwrap()
    .unwrap();
    assert_eq!(exported.version, 2);
    assert_eq!(
        exported.skills["tdd"].commit.as_deref(),
        Some(f.first.as_str())
    );
    assert_eq!(
        exported.skills["tdd"].hash.as_ref(),
        Some(&lock.skills["tdd"].hash)
    );
    assert!(
        kernel
            .plan_export_skills_lock(&f.proj)
            .unwrap()
            .changes
            .is_empty()
    );
}

#[test]
fn plain_init_writes_only_the_project_section() {
    let f = fixture("plain");
    let kernel = kernel(&f, Access::ReadWrite);
    // 不是 git 仓库（.git 是空目录）：没有提交者，默认链接。
    assert_eq!(kernel.suggest_mode(&f.proj).committers, 0);
    let plan = kernel.plan_init(&f.proj, false, None).unwrap();
    assert!(plan.notes.is_empty());
    kernel.apply(&plan).unwrap();
    let text = fs::read_to_string(f.proj.join("fangcun.toml")).unwrap();
    assert_eq!(
        text,
        "[project]\ntargets = [\"claude-code\", \"codex\"]\nmode    = \"link\"\n"
    );
}

#[test]
fn shared_repositories_are_suggested_vendor_mode() {
    let f = fixture("shared");
    let repo = f.base.join("shared");
    fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    let commit = |name: &str, email: &str, file: &str| {
        write(&repo.join(file), file);
        git(&repo, &["add", "-A"]);
        let author = format!("{name} <{email}>");
        git(&repo, &["commit", "-q", "-m", file, "--author", &author]);
    };
    commit("me", "me@example.com", "a");
    commit("dependabot[bot]", "bot@example.com", "b");
    let kernel = kernel(&f, Access::ReadWrite);
    let alone = kernel.suggest_mode(&repo);
    assert_eq!((alone.mode, alone.committers), (SyncMode::Link, 1));
    commit("Teammate", "Mate@Example.com", "c");
    commit("teammate", "mate@example.com", "d");
    let shared = kernel.suggest_mode(&repo);
    assert_eq!((shared.mode, shared.committers), (SyncMode::Vendor, 2));

    // 人选了别的方式时照人选的写。
    let plan = kernel
        .plan_init(&f.proj, false, Some(SyncMode::Vendor))
        .unwrap();
    kernel.apply(&plan).unwrap();
    let project = load_project(&f.proj, &f.home).unwrap().unwrap();
    assert_eq!(project.manifest.project.mode, Some(SyncMode::Vendor));
}

#[test]
fn a_project_with_a_manifest_is_registered_when_found() {
    let f = fixture("found");
    write(
        &f.proj.join("fangcun.toml"),
        "[project]\ntargets = [\"codex\"]\n",
    );

    // 只读模式下不登记。
    let read_only = kernel(&f, Access::ReadOnly);
    let listed = read_only.projects().unwrap();
    assert!(listed.iter().any(|p| p.name == "proj" && !p.registered));
    assert!(!f.home.projects_file().exists());

    let kernel = kernel(&f, Access::ReadWrite);
    let listed = kernel.projects().unwrap();
    assert!(listed.iter().any(|p| p.name == "proj" && p.registered));

    // 已有清单的项目再走登记：不反推，只补锁。
    let plan = kernel.plan_init(&f.proj, true, None).unwrap();
    assert_eq!(plan.title, "为 proj 补上清单与锁");
    let paths: Vec<_> = plan.changes.iter().map(|c| c.path.clone()).collect();
    assert_eq!(paths, [f.proj.join("fangcun.lock")]);
    assert!(plan.notes.iter().any(|n| n.text.contains("不再反推")));
}

#[test]
fn edited_copies_lock_the_newest_version_and_say_so() {
    let f = fixture("edited");
    write(
        &f.proj.join(".agents/skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\n改过\n",
    );
    let kernel = kernel(&f, Access::ReadWrite);
    let plan = kernel.plan_init(&f.proj, true, None).unwrap();
    assert!(
        plan.notes.iter().any(|n| n.tone == NoteTone::Warning
            && n.text.contains("tdd")
            && n.text.contains("对不上")),
        "{:?}",
        plan.notes
    );
}
