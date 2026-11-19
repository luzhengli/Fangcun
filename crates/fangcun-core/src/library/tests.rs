//! 资产库的测试：临时的 `~/.fangcun`，不碰真实磁盘。

use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;

use super::*;

struct Fixture {
    base: PathBuf,
    library: Library,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn fixture(name: &str) -> Fixture {
    let base = std::env::temp_dir().join(format!("fangcun-library-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    let library = Library::open(&FangcunHome::at(base.join(".fangcun"))).unwrap();
    Fixture { base, library }
}

/// 一个带脚本、软链、`metadata.json` 和 `.git` 的 skill。
fn skill(f: &Fixture, dir: &str, body: &str) -> PathBuf {
    let root = f.base.join(dir);
    fs::create_dir_all(root.join("scripts")).unwrap();
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(
        root.join("SKILL.md"),
        format!("---\nname: tdd\n---\n{body}\n"),
    )
    .unwrap();
    fs::write(root.join("scripts/run.sh"), "echo hi\n").unwrap();
    fs::set_permissions(
        root.join("scripts/run.sh"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    fs::write(root.join("metadata.json"), "{}").unwrap();
    fs::write(root.join(".git/HEAD"), "ref").unwrap();
    let _ = fs::remove_file(root.join("alias.md"));
    symlink("SKILL.md", root.join("alias.md")).unwrap();
    root
}

fn github() -> Origin {
    Origin {
        source: Some(SourceRef("github:obra/superpowers".to_owned())),
        commit: Some("b7e9f0c2".to_owned()),
        tag: None,
    }
}

#[test]
fn opening_and_listing_an_empty_library_touches_nothing() {
    let f = fixture("empty");
    assert!(f.library.entries().unwrap().is_empty());
    assert_eq!(
        f.library.locate(&ContentHash("ab".to_owned())).unwrap(),
        None
    );
    assert!(!f.base.join(".fangcun").exists(), "列出空库不建索引");
}

#[test]
fn stores_a_skill_under_its_content_hash() {
    let f = fixture("skill");
    let from = skill(&f, "src/tdd", "Test first.");
    let version = f
        .library
        .ingest("tdd", AssetKind::Skill, &from, &github())
        .unwrap();
    assert_eq!(version.hash, hash_dir(&from), "正本的哈希就是源目录的哈希");
    assert_eq!(version.commit.as_deref(), Some("b7e9f0c2"));

    let stored = f.library.locate(&version.hash).unwrap().unwrap();
    assert_eq!(hash_dir(&stored), version.hash);
    assert!(stored.join("SKILL.md").is_file());
    assert!(!stored.join(".git").exists());
    assert!(!stored.join("metadata.json").exists());
    let alias = stored.join("alias.md");
    assert!(
        !fs::symlink_metadata(&alias)
            .unwrap()
            .file_type()
            .is_symlink(),
        "软链存成普通文件"
    );
    let mode = fs::metadata(stored.join("scripts/run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o111, 0o111, "保留可执行权限");
    assert!(
        !f.base
            .join(".fangcun/library/tmp")
            .read_dir()
            .unwrap()
            .any(|_| true),
        "暂存目录清空"
    );

    let entries = f.library.entries().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "tdd");
    assert!(entries[0].has_scripts);
    assert_eq!(entries[0].versions, [version]);
}

#[test]
fn keeps_versions_and_never_duplicates_content() {
    let f = fixture("versions");
    let v1 = skill(&f, "a/tdd", "v1");
    let first = f
        .library
        .ingest("tdd", AssetKind::Skill, &v1, &github())
        .unwrap();
    // 同样的内容换个地方再存：不复制、不重复记版本。
    let copy = skill(&f, "b/tdd", "v1");
    let again = f
        .library
        .ingest("tdd", AssetKind::Skill, &copy, &github())
        .unwrap();
    assert_eq!(again.hash, first.hash);
    // 改了内容：同一项资产多一个版本。
    let second = skill(&f, "a/tdd", "v2");
    let changed = f
        .library
        .ingest("tdd", AssetKind::Skill, &second, &github())
        .unwrap();
    assert_ne!(changed.hash, first.hash);
    // 同名但来源未知：另一项资产，内容相同也只存一份正本。
    f.library
        .ingest("tdd", AssetKind::Skill, &copy, &Origin::default())
        .unwrap();

    let entries = f.library.entries().unwrap();
    assert_eq!(entries.len(), 2);
    let from_github = entries.iter().find(|e| e.source.is_some()).unwrap();
    let hashes: Vec<_> = from_github
        .versions
        .iter()
        .map(|v| v.hash.clone())
        .collect();
    assert_eq!(hashes, [first.hash, changed.hash]);
    let objects = fs::read_dir(f.base.join(".fangcun/library/objects"))
        .unwrap()
        .count();
    assert_eq!(objects, 2);
}

#[test]
fn stores_instructions_as_files_and_defers_mcp() {
    let f = fixture("instruction");
    let file = f.base.join("AGENTS.md");
    fs::write(&file, "# 规则\r\n").unwrap();
    let version = f
        .library
        .ingest(
            "AGENTS.md",
            AssetKind::Instruction,
            &file,
            &Origin::default(),
        )
        .unwrap();
    let stored = f.library.locate(&version.hash).unwrap().unwrap();
    assert_eq!(
        fs::read(stored).unwrap(),
        b"# \xe8\xa7\x84\xe5\x88\x99\r\n",
        "指令原样存，不换行尾"
    );

    let refused = f
        .library
        .ingest("github", AssetKind::Mcp, &file, &Origin::default());
    assert!(matches!(refused, Err(Error::NotImplemented(_))));
}

#[test]
fn locate_ignores_hashes_that_are_not_hex() {
    let f = fixture("locate");
    fs::create_dir_all(f.base.join(".fangcun/library/objects")).unwrap();
    fs::create_dir_all(f.base.join(".fangcun/library/secret")).unwrap();
    assert_eq!(
        f.library
            .locate(&ContentHash("../secret".to_owned()))
            .unwrap(),
        None
    );
    assert_eq!(f.library.locate(&ContentHash(String::new())).unwrap(), None);
}

#[test]
fn concurrent_ingests_all_reach_the_index() {
    let f = fixture("concurrent");
    let sources: Vec<PathBuf> = (0..8)
        .map(|i| skill(&f, &format!("s{i}/tdd"), &format!("v{i}")))
        .collect();
    std::thread::scope(|scope| {
        for (i, from) in sources.iter().enumerate() {
            let library = f.library.clone();
            scope.spawn(move || {
                library
                    .ingest(
                        &format!("skill-{i}"),
                        AssetKind::Skill,
                        from,
                        &Origin::default(),
                    )
                    .unwrap();
            });
        }
    });
    assert_eq!(f.library.entries().unwrap().len(), 8);
}

#[test]
fn staging_names_are_never_reused() {
    let f = fixture("staging");
    let first = f.library.staging().unwrap();
    fs::remove_dir(&first).unwrap();
    let second = f.library.staging().unwrap();
    assert_ne!(first, second, "删掉的暂存目录名不能再发给别人");
}
