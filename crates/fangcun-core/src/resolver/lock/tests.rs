//! 写锁的测试：临时目录里造 git 仓库冒充 GitHub，经 `Kernel` 从清单一直走到写出锁文件。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::Error;
use crate::journal::RecordKind;
use crate::kernel::{Access, Kernel};
use crate::manifest::{Layer, Lock, load_project};
use crate::paths::FangcunHome;

struct Fixture {
    base: PathBuf,
    home: FangcunHome,
    proj: PathBuf,
    work: PathBuf,
    first: String,
    second: String,
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
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
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

fn fixture(name: &str) -> Fixture {
    let base = std::env::temp_dir().join(format!("fangcun-lock-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let work = base.join("work");
    fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    write(
        &work.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\nv1\n",
    );
    write(
        &work.join("skills/review/SKILL.md"),
        "---\nname: review\n---\nr\n",
    );
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "one"]);
    git(&work, &["tag", "v1.0.0"]);
    let first = git(&work, &["rev-parse", "HEAD"]);
    write(
        &work.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\nv2\n",
    );
    git(&work, &["commit", "-q", "-am", "two"]);
    let second = git(&work, &["rev-parse", "HEAD"]);
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

    let proj = base.join("home/proj");
    fs::create_dir_all(proj.join(".git")).unwrap();
    write(&proj.join("AGENTS.md"), "# 规则\n");
    let home = FangcunHome::at(base.join("home/.fangcun"));
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
        proj,
        work,
        first,
        second,
    }
}

fn kernel(f: &Fixture) -> Kernel {
    Kernel::with_home(f.home.clone(), Access::ReadWrite)
        .unwrap()
        .with_user_home(f.base.join("home"))
        .with_github_base(format!("file://{}", f.base.join("github").display()))
}

const MANIFEST: &str = r#"
[project]
targets = ["claude-code", "codex"]
bundles = ["工程基线"]

[skills]
tdd = { source = "github:acme/skills", track = "main" }

[mcp.github]
command = "npx"
args    = ["-y", "@modelcontextprotocol/server-github"]
env     = { GITHUB_TOKEN = "secret:github-token" }

[instructions]
canonical = "AGENTS.md"
mirror    = ["CLAUDE.md"]
"#;

const BUNDLE: &str = r#"
[bundle]
name = "工程基线"

[skills]
review = { source = "github:acme/skills", pin = "v1.0.0" }
tdd    = { source = "github:acme/skills", pin = "v1.0.0" }
"#;

const LOCAL: &str = r#"
[skills]
mine = { source = "github:acme/skills", track = "main", path = "skills/tdd" }

[dev]
skills = ["tdd"]
"#;

fn setup(f: &Fixture) {
    write(&f.proj.join("fangcun.toml"), MANIFEST);
    write(&f.home.bundle_file("工程基线"), BUNDLE);
    write(&f.proj.join("fangcun.local.toml"), LOCAL);
}

#[test]
fn layers_merge_with_the_project_over_bundles_and_personal_on_top() {
    let f = fixture("layers");
    setup(&f);
    let project = load_project(&f.proj, &f.home).unwrap().unwrap();
    let team = project.team_declarations();
    assert_eq!(team.skills["tdd"].layer, Layer::Project, "项目清单盖过套装");
    assert_eq!(
        team.skills["review"].layer,
        Layer::Bundle("工程基线".to_owned())
    );
    assert!(
        !team.skills.contains_key("mine"),
        "个人加的项不进全队的声明"
    );
    let local = project.local_declarations();
    assert_eq!(local.skills["mine"].layer, Layer::Personal);
    assert_eq!(local.dev, ["tdd"]);
}

#[test]
fn each_file_may_only_hold_its_own_sections() {
    let f = fixture("misplaced");
    write(&f.proj.join("fangcun.toml"), "[dev]\nskills = [\"x\"]\n");
    assert!(
        load_project(&f.proj, &f.home)
            .unwrap_err()
            .to_string()
            .contains("[dev]")
    );
    write(&f.proj.join("fangcun.toml"), MANIFEST);
    write(
        &f.proj.join("fangcun.local.toml"),
        "[project]\nbudget = 1\n",
    );
    assert!(
        load_project(&f.proj, &f.home)
            .unwrap_err()
            .to_string()
            .contains("budget")
    );
    write(&f.proj.join("fangcun.local.toml"), "");
    write(
        &f.home.bundle_file("工程基线"),
        "[project]\ntargets = [\"codex\"]\n",
    );
    assert!(load_project(&f.proj, &f.home).is_err());
    // 本机没有套装文件：不报错，按锁还原（7.4）。
    let _ = fs::remove_file(f.home.bundle_file("工程基线"));
    let project = load_project(&f.proj, &f.home).unwrap().unwrap();
    assert_eq!(project.bundles[0].bundle, None);
}

#[test]
fn writes_the_lock_for_bundles_and_the_project() {
    let f = fixture("write");
    setup(&f);
    let kernel = kernel(&f);
    let plan = kernel.plan_lock(&f.proj).unwrap();
    assert_eq!(plan.title, "更新 proj 的锁");
    assert_eq!(plan.changes.len(), 1);
    assert!(
        plan.changes[0].detail.starts_with("新增 "),
        "{}",
        plan.changes[0].detail
    );
    kernel.apply(&plan).unwrap();

    let lock = Lock::load(&f.proj).unwrap().unwrap();
    let tdd = &lock.skills["tdd"];
    assert_eq!(tdd.track.as_deref(), Some("main"));
    assert_eq!(tdd.commit.as_deref(), Some(f.second.as_str()));
    assert_eq!(tdd.path, "skills/tdd");
    assert_eq!(tdd.bundle, None);
    let review = &lock.skills["review"];
    assert_eq!(review.commit.as_deref(), Some(f.first.as_str()));
    assert_eq!(review.tag.as_deref(), Some("v1.0.0"));
    assert_eq!(review.bundle.as_deref(), Some("工程基线"));
    assert!(!lock.skills.contains_key("mine"));
    assert_eq!(lock.mcp["github"].hash.0.len(), 64);
    assert_eq!(lock.instructions["AGENTS.md"].path, "AGENTS.md");
    let text = fs::read_to_string(f.proj.join("fangcun.lock")).unwrap();
    assert!(text.starts_with("# fangcun.lock"));
    assert!(
        !text.contains("secret"),
        "锁里只有哈希，不出现密钥引用以外的东西"
    );

    let records = kernel
        .log(&crate::journal::RecordFilter::default())
        .unwrap();
    assert_eq!(records[0].kind, RecordKind::Sync);
    assert_eq!(records[0].command.as_deref(), Some("fangcun lock"));
}

#[test]
fn unchanged_constraints_keep_their_versions_without_the_network() {
    let f = fixture("keep");
    setup(&f);
    let kernel = kernel(&f);
    kernel.apply(&kernel.plan_lock(&f.proj).unwrap()).unwrap();
    let before = Lock::load(&f.proj).unwrap().unwrap();

    // 上游有了新提交，GitHub 也连不上了：约束没变的项照旧，锁不变。
    write(
        &f.work.join("skills/tdd/SKILL.md"),
        "---\nname: tdd\n---\nv3\n",
    );
    git(&f.work, &["commit", "-q", "-am", "three"]);
    let offline = Kernel::with_home(f.home.clone(), Access::ReadWrite)
        .unwrap()
        .with_user_home(f.base.join("home"))
        .with_github_base("file:///nonexistent");
    let plan = offline.plan_lock(&f.proj).unwrap();
    assert!(plan.changes.is_empty(), "{:?}", plan.changes);
    assert_eq!(plan.title, "proj 的锁已是最新");

    // 换了约束：重新解析这一项；删掉的项从锁里去掉。
    let manifest = MANIFEST
        .replace(r#"track = "main""#, r#"pin = "v1.0.0""#)
        .replace("[mcp.github]\ncommand = \"npx\"\nargs    = [\"-y\", \"@modelcontextprotocol/server-github\"]\nenv     = { GITHUB_TOKEN = \"secret:github-token\" }\n", "");
    write(&f.proj.join("fangcun.toml"), &manifest);
    let plan = kernel.plan_lock(&f.proj).unwrap();
    assert_eq!(plan.changes[0].detail, "更新 tdd；去掉 github");
    kernel.apply(&plan).unwrap();
    let after = Lock::load(&f.proj).unwrap().unwrap();
    assert_eq!(
        after.skills["tdd"].commit.as_deref(),
        Some(f.first.as_str())
    );
    assert_eq!(after.skills["review"], before.skills["review"]);
    assert!(after.mcp.is_empty());
}

#[test]
fn refuses_unregistered_projects_missing_manifests_and_double_constraints() {
    let f = fixture("refuse");
    let kernel = kernel(&f);
    let missing = kernel.plan_lock(&f.proj).unwrap_err();
    assert!(missing.to_string().contains("fangcun init"), "{missing}");

    let other = f.base.join("home/other");
    fs::create_dir_all(&other).unwrap();
    write(&other.join("fangcun.toml"), MANIFEST);
    assert!(
        kernel
            .plan_lock(&other)
            .unwrap_err()
            .to_string()
            .contains("还没登记")
    );

    write(
        &f.proj.join("fangcun.toml"),
        "[skills]\ntdd = { source = \"github:acme/skills\", pin = \"v1.0.0\", track = \"main\" }\n",
    );
    let both = kernel.plan_lock(&f.proj).unwrap_err();
    assert!(
        matches!(both, Error::Source(_)) && both.to_string().contains("只能写一个"),
        "{both}"
    );

    let read_only = Kernel::with_home(f.home.clone(), Access::ReadOnly).unwrap();
    assert!(matches!(
        read_only.plan_lock(&f.proj),
        Err(Error::ReadOnly(_))
    ));
}
