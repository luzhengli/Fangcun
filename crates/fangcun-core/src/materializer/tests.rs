//! 物化器骨架的测试：都经 `Kernel`，用临时的 `~/.fangcun` 和临时主目录，不碰真实磁盘。

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::snapshot::{self, SnapshotScope};
use super::*;
use crate::journal::{RecordFilter, RecordKind, RecordStatus};
use crate::kernel::{Access, Kernel};

struct Fixture {
    base: PathBuf,
    home: PathBuf,
    proj: PathBuf,
    kernel: Kernel,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn fixture(name: &str) -> Fixture {
    let base = std::env::temp_dir().join(format!("fangcun-apply-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let home = base.join("home");
    let proj = home.join("proj");
    fs::create_dir_all(proj.join(".git")).unwrap();
    fs::create_dir_all(home.join("other")).unwrap();
    let own = FangcunHome::at(home.join(".fangcun"));
    fs::create_dir_all(own.root()).unwrap();
    fs::write(
        own.projects_file(),
        format!(
            "[[project]]\npath = \"{}\"\nregistered = \"2026-11-19T00:00:00Z\"\n",
            proj.display()
        ),
    )
    .unwrap();
    let kernel = Kernel::with_home(own, Access::ReadWrite)
        .unwrap()
        .with_user_home(&home);
    Fixture {
        base,
        home,
        proj,
        kernel,
    }
}

fn builder(f: &Fixture) -> PlanBuilder {
    PlanBuilder::new(
        "测试写入",
        RecordKind::Sync,
        SnapshotScope::Project(f.proj.clone()),
    )
    .command("fangcun sync")
}

/// 目录树的样子：路径 → 类型、内容或链接目标、权限。`.fangcun` 不算。
fn tree(root: &Path) -> BTreeMap<PathBuf, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, String>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == ".fangcun") {
                continue;
            }
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            let meta = fs::symlink_metadata(&path).unwrap();
            let mode = meta.permissions().mode() & 0o7777;
            if meta.file_type().is_symlink() {
                out.insert(
                    rel,
                    format!("link {}", fs::read_link(&path).unwrap().display()),
                );
            } else if meta.is_dir() {
                out.insert(rel, format!("dir {mode:o}"));
                walk(root, &path, out);
            } else {
                out.insert(rel, format!("file {mode:o} {:?}", fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn snapshot_dir_exists(f: &Fixture) -> bool {
    f.kernel.home().snapshots_dir().exists()
}

#[test]
fn applies_with_a_snapshot_and_restores_byte_for_byte() {
    let f = fixture("roundtrip");
    fs::write(f.proj.join("AGENTS.md"), b"old\r\n").unwrap();
    fs::set_permissions(f.proj.join("AGENTS.md"), fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(f.proj.join("old.txt"), b"bye").unwrap();
    let before = tree(&f.home);

    let mut b = builder(&f);
    let skill = f.proj.join(".claude/skills/tdd/SKILL.md");
    b.write_file(&skill, b"---\nname: tdd\n---\n".to_vec(), "新增 tdd")
        .unwrap();
    b.write_file(&f.proj.join("AGENTS.md"), b"new\n".to_vec(), "改写正本")
        .unwrap();
    b.symlink(&f.proj.join("CLAUDE.md"), Path::new("AGENTS.md"), "镜像")
        .unwrap();
    b.remove(&f.proj.join("old.txt"), "删掉").unwrap();
    let plan = f.kernel.issue(b);
    let ops: Vec<ChangeOp> = plan.changes.iter().map(|c| c.op).collect();
    assert_eq!(
        ops,
        [
            ChangeOp::Add,
            ChangeOp::Modify,
            ChangeOp::Add,
            ChangeOp::Remove
        ]
    );

    let applied = f.kernel.apply(&plan).unwrap();
    assert_eq!(applied.snapshots, [SnapshotId(1)]);
    assert_eq!(fs::read(&skill).unwrap(), b"---\nname: tdd\n---\n");
    assert_eq!(fs::read(f.proj.join("AGENTS.md")).unwrap(), b"new\n");
    let mode = fs::metadata(f.proj.join("AGENTS.md"))
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "改写保留原来的权限");
    assert_eq!(
        fs::read_link(f.proj.join("CLAUDE.md")).unwrap(),
        Path::new("AGENTS.md")
    );
    assert!(!f.proj.join("old.txt").exists());

    let records = f.kernel.log(&RecordFilter::default()).unwrap();
    let record = records.iter().find(|r| r.kind == RecordKind::Sync).unwrap();
    assert_eq!(record.status, RecordStatus::Applied);
    assert_eq!(record.snapshot, Some(SnapshotId(1)));
    assert_eq!(record.project_label, "proj");
    assert_eq!(record.command.as_deref(), Some("fangcun sync"));

    // 按快照倒着还原，磁盘回到写入前，逐字节一致，连新建的上层目录也删掉了。
    let home = f.kernel.home().clone();
    let saved = snapshot::load(
        &home,
        &SnapshotScope::Project(f.proj.clone()),
        SnapshotId(1),
    )
    .unwrap();
    for entry in saved.entries.iter().rev() {
        snapshot::restore(&home, entry, &crate::secrets::MemoryStore::default()).unwrap();
    }
    assert_eq!(tree(&f.home), before);
}

#[test]
fn refuses_a_stale_plan_without_writing() {
    let f = fixture("stale");
    fs::write(f.proj.join("AGENTS.md"), b"v1").unwrap();
    let mut b = builder(&f);
    b.write_file(&f.proj.join("AGENTS.md"), b"v2".to_vec(), "改写")
        .unwrap();
    b.write_file(&f.proj.join("new.md"), b"x".to_vec(), "新增")
        .unwrap();
    let plan = f.kernel.issue(b);
    fs::write(f.proj.join("AGENTS.md"), b"edited by hand").unwrap();

    assert!(matches!(f.kernel.apply(&plan), Err(Error::StalePlan(_))));
    assert_eq!(
        fs::read(f.proj.join("AGENTS.md")).unwrap(),
        b"edited by hand"
    );
    assert!(!f.proj.join("new.md").exists());
    assert!(!snapshot_dir_exists(&f));
}

#[test]
fn refuses_out_of_scope_paths_without_writing() {
    let f = fixture("scope");
    let mut b = builder(&f);
    b.write_file(&f.proj.join("ok.md"), b"x".to_vec(), "项目里")
        .unwrap();
    b.write_file(&f.home.join("other/file"), b"x".to_vec(), "范围外")
        .unwrap();
    let plan = f.kernel.issue(b);

    assert!(matches!(f.kernel.apply(&plan), Err(Error::OutOfScope(_))));
    assert!(!f.proj.join("ok.md").exists());
    assert!(!snapshot_dir_exists(&f));
}

#[test]
fn only_plans_issued_by_the_kernel_apply_and_only_once() {
    let f = fixture("once");
    let mut b = builder(&f);
    b.write_file(&f.proj.join("a.md"), b"a".to_vec(), "新增")
        .unwrap();
    let plan = f.kernel.issue(b);

    // 经过 IPC 来回一趟，动作不在 JSON 里，凭编号找回。
    let json = serde_json::to_string(&plan).unwrap();
    assert!(!json.contains("body"));
    let mut returned: ChangePlan = serde_json::from_str(&json).unwrap();
    returned.title = "被改过的标题".to_owned();
    returned.changes.clear();
    f.kernel.apply(&returned).unwrap();
    assert_eq!(fs::read(f.proj.join("a.md")).unwrap(), b"a");

    assert!(matches!(
        f.kernel.apply(&returned),
        Err(Error::PlanNotFound(_))
    ));
}

#[test]
fn a_failure_midway_restores_what_was_written() {
    let f = fixture("midway");
    let locked = f.proj.join("locked");
    fs::create_dir(&locked).unwrap();
    let before = tree(&f.home);
    let mut b = builder(&f);
    b.write_file(&f.proj.join("first.md"), b"1".to_vec(), "先写")
        .unwrap();
    b.write_file(&f.proj.join("nested/dir/x.md"), b"2".to_vec(), "建目录")
        .unwrap();
    b.write_file(&locked.join("x.md"), b"3".to_vec(), "写不进去")
        .unwrap();
    let plan = f.kernel.issue(b);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();

    let result = f.kernel.apply(&plan);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(result, Err(Error::Io { .. })),
        "得到的是 {result:?}"
    );
    assert_eq!(tree(&f.home), before);
}

#[test]
fn a_plan_that_registers_a_project_may_write_into_it() {
    let f = fixture("register");
    let fresh = f.home.join("fresh");
    fs::create_dir(&fresh).unwrap();

    let mut without = builder(&f);
    without
        .write_file(&fresh.join("fangcun.toml"), b"".to_vec(), "清单")
        .unwrap();
    let plan = f.kernel.issue(without);
    assert!(matches!(f.kernel.apply(&plan), Err(Error::OutOfScope(_))));

    let mut with = PlanBuilder::new(
        "登记",
        RecordKind::Sync,
        SnapshotScope::Project(fresh.clone()),
    );
    with.registers(&fresh);
    with.write_file(&fresh.join("fangcun.toml"), b"".to_vec(), "清单")
        .unwrap();
    let plan = f.kernel.issue(with);
    assert_eq!(f.kernel.apply(&plan).unwrap().snapshots, [SnapshotId(1)]);
    assert!(fresh.join("fangcun.toml").exists());
}

#[test]
fn global_writes_get_their_own_snapshot_numbering() {
    let f = fixture("global");
    let mut b = builder(&f);
    b.write_file(&f.proj.join("a.md"), b"a".to_vec(), "项目")
        .unwrap();
    b.write_file(
        &f.home.join(".claude/skills/x/SKILL.md"),
        b"x".to_vec(),
        "全局",
    )
    .unwrap();
    let plan = f.kernel.issue(b);
    let applied = f.kernel.apply(&plan).unwrap();
    assert_eq!(applied.snapshots, [SnapshotId(1), SnapshotId(1)]);
    let home = f.kernel.home();
    assert!(home.snapshots_dir().join("global/1.toml").exists());

    let records = f.kernel.log(&RecordFilter::default()).unwrap();
    assert_eq!(records[0].project_label, "proj 等 2 个范围");
    assert_eq!(records[0].project, None);
}

#[test]
fn read_only_kernel_refuses_to_apply_an_issued_plan() {
    let f = fixture("readonly");
    let kernel = Kernel::with_home(f.kernel.home().clone(), Access::ReadOnly)
        .unwrap()
        .with_user_home(&f.home);
    let mut b = builder(&f);
    b.write_file(&f.proj.join("a.md"), b"a".to_vec(), "新增")
        .unwrap();
    let plan = kernel.issue(b);
    assert!(matches!(kernel.apply(&plan), Err(Error::ReadOnly(_))));
    assert!(!f.proj.join("a.md").exists());
}
