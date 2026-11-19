//! 来源的测试：临时目录里造 git 仓库，用 `file://` 地址冒充 GitHub，不联网。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::*;
use crate::library::content::hash_dir;

struct Fixture {
    base: PathBuf,
    work: PathBuf,
    bare: PathBuf,
    library: Library,
    sources: Sources,
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

fn skill_md(name: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: d\n---\n{body}\n")
}

/// 工作仓库 acme/skills：第一个提交打标签 v1.0.0，第二个提交改了 tdd。裸仓库是它的克隆。
fn fixture(name: &str) -> (Fixture, String, String) {
    let base = std::env::temp_dir().join(format!("fangcun-source-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let work = base.join("work");
    fs::create_dir_all(&work).unwrap();
    git(&work, &["init", "-q", "-b", "main"]);
    write(&work.join("skills/tdd/SKILL.md"), &skill_md("tdd", "v1"));
    write(&work.join("skills/other/SKILL.md"), &skill_md("other", "o"));
    write(&work.join("README.md"), "readme");
    git(&work, &["add", "-A"]);
    git(&work, &["commit", "-q", "-m", "one"]);
    git(&work, &["tag", "v1.0.0"]);
    let first = git(&work, &["rev-parse", "HEAD"]);
    write(&work.join("skills/tdd/SKILL.md"), &skill_md("tdd", "v2"));
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

    let home = FangcunHome::at(base.join(".fangcun"));
    let library = Library::open(&home).unwrap();
    let sources =
        Sources::open(&home).with_github_base(format!("file://{}", base.join("github").display()));
    (
        Fixture {
            base,
            work,
            bare,
            library,
            sources,
        },
        first,
        second,
    )
}

fn github() -> SourceRef {
    SourceRef("github:acme/skills".to_owned())
}

#[test]
fn parses_the_two_source_forms_and_rejects_the_rest() {
    assert_eq!(
        Source::parse(&github()).unwrap(),
        Source::Github {
            owner: "acme".to_owned(),
            repo: "skills".to_owned()
        }
    );
    assert_eq!(
        Source::parse(&SourceRef("local:luyao-skills".to_owned())).unwrap(),
        Source::Local {
            name: "luyao-skills".to_owned()
        }
    );
    for bad in [
        "github:acme",
        "github:../x/y",
        "github:-x/y",
        "local:",
        "local:a/b",
        "npm:x",
        "acme/skills",
    ] {
        assert!(Source::parse(&SourceRef(bad.to_owned())).is_err(), "{bad}");
    }
}

#[test]
fn track_resolves_the_branch_tip_and_stores_that_copy() {
    let (f, _, second) = fixture("track");
    let track = VersionPolicy::Track("main".to_owned());
    let resolved = f
        .sources
        .resolve_skill(&f.library, "tdd", &github(), Some(&track), None)
        .unwrap();
    assert_eq!(resolved.commit.as_deref(), Some(second.as_str()));
    assert_eq!(resolved.tag, None);
    assert_eq!(resolved.path, "skills/tdd");
    assert_eq!(resolved.version.hash, hash_dir(&f.work.join("skills/tdd")));
    let stored = f.library.locate(&resolved.version.hash).unwrap().unwrap();
    assert!(
        fs::read_to_string(stored.join("SKILL.md"))
            .unwrap()
            .contains("v2")
    );
    let entry = &f.library.entries().unwrap()[0];
    assert_eq!(entry.source, Some(github()));
    assert_eq!(entry.versions[0].commit.as_deref(), Some(second.as_str()));
    assert!(
        f.base
            .join(".fangcun/git/github.com/acme/skills.git/HEAD")
            .exists()
    );
}

#[test]
fn pin_resolves_tags_and_commits_and_finds_the_tag_of_a_commit() {
    let (f, first, _) = fixture("pin");
    let by_tag = VersionPolicy::Pin("v1.0.0".to_owned());
    let resolved = f
        .sources
        .resolve_skill(&f.library, "tdd", &github(), Some(&by_tag), None)
        .unwrap();
    assert_eq!(resolved.commit.as_deref(), Some(first.as_str()));
    assert_eq!(resolved.tag.as_deref(), Some("v1.0.0"));

    let by_commit = VersionPolicy::Pin(first[..7].to_owned());
    let again = f
        .sources
        .resolve_skill(&f.library, "tdd", &github(), Some(&by_commit), None)
        .unwrap();
    assert_eq!(again.commit.as_deref(), Some(first.as_str()));
    assert_eq!(again.tag.as_deref(), Some("v1.0.0"), "按提交反查到标签");
    assert_eq!(again.version, resolved.version);

    for bad in ["v9", "--upload-pack=touch", "zzzzzzz"] {
        let policy = VersionPolicy::Pin(bad.to_owned());
        assert!(
            f.sources
                .resolve_skill(&f.library, "tdd", &github(), Some(&policy), None)
                .is_err()
        );
    }
}

#[test]
fn later_resolves_fetch_new_commits() {
    let (f, _, _) = fixture("fetch");
    let track = VersionPolicy::Track("main".to_owned());
    f.sources
        .resolve_skill(&f.library, "tdd", &github(), Some(&track), None)
        .unwrap();
    write(&f.work.join("skills/tdd/SKILL.md"), &skill_md("tdd", "v3"));
    git(&f.work, &["commit", "-q", "-am", "three"]);
    let third = git(&f.work, &["rev-parse", "HEAD"]);
    git(&f.work, &["push", "-q", f.bare.to_str().unwrap(), "main"]);

    let resolved = f
        .sources
        .resolve_skill(&f.library, "tdd", &github(), Some(&track), None)
        .unwrap();
    assert_eq!(resolved.commit.as_deref(), Some(third.as_str()));
    assert_eq!(f.library.entries().unwrap()[0].versions.len(), 2);
}

#[test]
fn asks_for_a_path_when_the_name_is_ambiguous() {
    let (f, _, _) = fixture("ambiguous");
    write(&f.work.join("extra/tdd/SKILL.md"), &skill_md("tdd", "fork"));
    git(&f.work, &["add", "-A"]);
    git(&f.work, &["commit", "-q", "-m", "fork"]);
    git(&f.work, &["push", "-q", f.bare.to_str().unwrap(), "main"]);

    let error = f
        .sources
        .resolve_skill(&f.library, "tdd", &github(), None, None)
        .unwrap_err();
    let message = error.to_string();
    assert!(
        message.contains("extra/tdd") && message.contains("skills/tdd") && message.contains("path"),
        "{message}"
    );

    let picked = f
        .sources
        .resolve_skill(&f.library, "tdd", &github(), None, Some("extra/tdd"))
        .unwrap();
    assert_eq!(picked.path, "extra/tdd");
    assert!(
        f.sources
            .resolve_skill(&f.library, "missing", &github(), None, None)
            .is_err()
    );
    assert!(
        f.sources
            .resolve_skill(&f.library, "tdd", &github(), None, Some("../x"))
            .is_err()
    );
}

#[test]
fn local_sources_use_the_recorded_directory() {
    let (f, first, _) = fixture("local");
    let local = SourceRef("local:my-skills".to_owned());
    let missing = f
        .sources
        .resolve_skill(&f.library, "tdd", &local, None, None)
        .unwrap_err();
    assert!(missing.to_string().contains("本机没有来源 my-skills"));

    f.sources.remember_local("my-skills", &f.work).unwrap();
    assert_eq!(f.sources.local_dir("my-skills").unwrap(), f.work);
    // 工作目录里还没提交的改动：没写版本时取现在的样子，没有提交。
    write(
        &f.work.join("skills/tdd/SKILL.md"),
        &skill_md("tdd", "draft"),
    );
    let live = f
        .sources
        .resolve_skill(&f.library, "tdd", &local, None, None)
        .unwrap();
    assert_eq!(live.commit, None);
    assert_eq!(live.version.hash, hash_dir(&f.work.join("skills/tdd")));

    let pinned = VersionPolicy::Pin("v1.0.0".to_owned());
    let resolved = f
        .sources
        .resolve_skill(&f.library, "tdd", &local, Some(&pinned), None)
        .unwrap();
    assert_eq!(resolved.commit.as_deref(), Some(first.as_str()));
}
