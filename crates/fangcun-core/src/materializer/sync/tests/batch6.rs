//! 批 6 的测试：MCP 入库（M1-20）、从全局移除（M1-34、M1-02）、`fangcun add`（M1-22）、
//! 体检的处理进度（M1-28）。和上一层共用临时主目录、冒充 GitHub 的仓库；钥匙串用内存里的。

use std::fs;
use std::sync::Arc;

use super::{TDD_DECL, all_text, fixture, kernel, lock_and_sync, tree, write};
use crate::Error;
use crate::global::GlobalItemRef;
use crate::journal::RecordFilter;
use crate::model::{AssetKind, ToolId};
use crate::scanner::FindingStatus;
use crate::secrets::{MemoryStore, SecretStore as _};

const KEY: &str = "sk-abcdefghijklmnop";

fn claude() -> ToolId {
    ToolId::from(ToolId::CLAUDE_CODE)
}

fn item(name: &str) -> GlobalItemRef {
    GlobalItemRef {
        kind: None,
        name: name.to_owned(),
    }
}

/// 全局层 `~/.claude.json` 里一个带明文密钥的 MCP：没收编时不许移除；收编后资产库存的是引用版本，
/// 明文进钥匙串；移除只删这一个键，快照和活动记录里没有明文，回滚逐字节一致。
/// 再把它从资产库加进项目清单（`fangcun add`）。
#[test]
fn global_mcp_is_adopted_with_references_removed_by_key_and_rolled_back() {
    let f = fixture("b6-mcp");
    let store = Arc::new(MemoryStore::default());
    let kernel = kernel(&f).with_secret_store(store.clone());
    let claude_json = f.user_home.join(".claude.json");
    write(
        &claude_json,
        &format!(
            "{{\n  \"numStartups\": 3,\n  \"mcpServers\": {{\n    \"context7\": {{ \"command\": \"npx\", \"args\": [\"c7\"], \"env\": {{ \"API_KEY\": \"{KEY}\" }} }},\n    \"keep\": {{ \"command\": \"k\" }}\n  }}\n}}\n"
        ),
    );
    let original = fs::read(&claude_json).unwrap();

    let refused = kernel.plan_remove_global(Some(&claude()), &[item("context7")]);
    assert!(
        matches!(&refused, Err(Error::NotAdopted(n)) if n == "context7"),
        "{refused:?}"
    );
    let overview = kernel.global_overview(&claude()).unwrap();
    let row = overview.rows.iter().find(|r| r.name == "context7").unwrap();
    assert!(!row.in_library);

    // 收编：资产库里是引用版本，明文在应用时进钥匙串，配置不动。
    let adopt = kernel.plan_adopt("context7").unwrap();
    assert!(
        adopt.title.starts_with("收录 MCP context7"),
        "{}",
        adopt.title
    );
    assert!(
        adopt
            .changes
            .iter()
            .any(|c| c.path.to_string_lossy().contains("context7-api-key")),
        "{:?}",
        adopt.changes
    );
    assert!(!serde_json::to_string(&adopt).unwrap().contains(KEY));
    kernel.apply(&adopt).unwrap();
    assert_eq!(
        store.get("context7-api-key").unwrap().unwrap().expose(),
        KEY
    );
    assert_eq!(fs::read(&claude_json).unwrap(), original, "配置原样不动");
    let entries = crate::library::Library::open(&f.home)
        .unwrap()
        .entries()
        .unwrap();
    let mcp = entries
        .iter()
        .find(|e| e.kind == AssetKind::Mcp && e.name == "context7")
        .unwrap();
    assert_eq!(mcp.versions.len(), 1);
    assert!(!all_text(&f.home.library_dir()).contains(KEY));

    let overview = kernel.global_overview(&claude()).unwrap();
    let row = overview.rows.iter().find(|r| r.name == "context7").unwrap();
    assert!(row.in_library);

    // 从全局移除：只删 mcpServers.context7。
    let plan = kernel
        .plan_remove_global(Some(&claude()), &[item("context7")])
        .unwrap();
    assert_eq!(plan.title, "从全局层移除 context7");
    assert!(!plan.blocked, "{:?}", plan.notes);
    assert_eq!(plan.changes.len(), 1, "{:?}", plan.changes);
    assert!(plan.changes[0].detail.contains("资产库保留正本"));
    let applied = kernel.apply(&plan).unwrap();
    let after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&claude_json).unwrap()).unwrap();
    assert!(after["mcpServers"]["context7"].is_null());
    assert_eq!(after["mcpServers"]["keep"]["command"], "k");
    assert_eq!(after["numStartups"], 3);
    assert!(!all_text(&f.home.snapshots_dir()).contains(KEY));
    assert!(!all_text(&f.home.journal_dir()).contains(KEY));

    // 回滚：明文从钥匙串取回，逐字节一致。
    let undo = kernel.plan_rollback_record(&applied.record).unwrap();
    assert!(!undo.blocked, "{:?}", undo.notes);
    kernel.apply(&undo).unwrap();
    assert_eq!(fs::read(&claude_json).unwrap(), original);

    // 从资产库加进项目：清单写 secret: 引用，提交的 .mcp.json 只有环境变量引用。
    lock_and_sync(&kernel, &f, &super::manifest("link", TDD_DECL, false));
    let add = kernel.plan_add(&f.proj, "context7").unwrap();
    assert!(add.title.contains("context7"), "{}", add.title);
    kernel.apply(&add).unwrap();
    let manifest = fs::read_to_string(f.proj.join("fangcun.toml")).unwrap();
    assert!(manifest.contains("[mcp.context7]"), "{manifest}");
    assert!(manifest.contains("secret:context7-api-key"), "{manifest}");
    let committed = fs::read_to_string(f.proj.join(".mcp.json")).unwrap();
    assert!(
        committed.contains("context7") && !committed.contains(KEY),
        "{committed}"
    );
    assert!(
        fs::read_to_string(f.proj.join("fangcun.lock"))
            .unwrap()
            .contains("context7")
    );
}

/// 全局 skill：体检「重复」一键收编后这一条算已处理，回滚后回到待处理（M1-28）；收编前不许
/// 从全局移除，收编后只删 Claude Code 的那一处，列出项目层同名的项目，回滚逐字节一致。
#[test]
fn global_skill_removal_needs_adoption_and_checkup_progress_follows_rollback() {
    let f = fixture("b6-skill");
    let kernel = kernel(&f);
    let body = "---\nname: harness\n---\nsame\n";
    write(&f.user_home.join(".agents/skills/harness/SKILL.md"), body);
    write(&f.user_home.join(".claude/skills/harness/SKILL.md"), body);
    write(
        &f.proj.join(".claude/skills/harness/SKILL.md"),
        "---\nname: harness\n---\nforked\n",
    );

    let report = kernel.doctor().unwrap();
    let id = "duplicate:harness";
    let finding = report.findings.iter().find(|x| x.id == id).unwrap();
    assert_eq!(finding.status, FindingStatus::Pending);
    assert!(finding.ready && finding.record.is_none());
    assert_eq!(report.progress.handled, 0);
    assert!(report.progress.total >= 1);

    let refused = kernel.plan_remove_global(Some(&claude()), &[item("harness")]);
    assert!(matches!(refused, Err(Error::NotAdopted(_))), "{refused:?}");

    // 一键处理：记录带着发现的编号，报告里这一条已处理。
    let plan = kernel.plan_finding(id).unwrap();
    let applied = kernel.apply(&plan).unwrap();
    let record = kernel
        .log(&RecordFilter::default())
        .unwrap()
        .into_iter()
        .find(|r| r.id == applied.record)
        .unwrap();
    assert_eq!(record.finding.as_deref(), Some(id));
    let report = kernel.doctor().unwrap();
    let finding = report.findings.iter().find(|x| x.id == id).unwrap();
    assert_eq!(finding.status, FindingStatus::Resolved);
    assert_eq!(finding.record.as_deref(), Some(applied.record.as_str()));
    assert_eq!(report.progress.handled, 1);

    // 撤销：回到待处理。
    let undo = kernel.plan_rollback_record(&applied.record).unwrap();
    kernel.apply(&undo).unwrap();
    let report = kernel.doctor().unwrap();
    let finding = report.findings.iter().find(|x| x.id == id).unwrap();
    assert_eq!(finding.status, FindingStatus::Pending);
    assert_eq!(report.progress.handled, 0);

    // 再收编一次；扫描还是旧的也不会去删资产库里的文件。
    let again = kernel.plan_finding(id).unwrap();
    kernel.apply(&again).unwrap();
    let stale = kernel.plan_adopt("harness");
    assert!(stale.is_err(), "已经是链接，没有可收编的：{stale:?}");
    let objects = f.home.library_dir().join("objects");
    let before_library = tree(&objects);

    // 从全局移除（扫描还是旧的，按磁盘判断是软链）：只删 Claude Code 那一处。
    let plan = kernel
        .plan_remove_global(Some(&claude()), &[item("harness")])
        .unwrap();
    assert_eq!(plan.title, "从全局层移除 harness");
    assert_eq!(
        plan.changes
            .iter()
            .filter(|c| c.path.starts_with(f.user_home.join(".claude")))
            .count(),
        1,
        "{:?}",
        plan.changes
    );
    let texts: Vec<&str> = plan.notes.iter().map(|n| n.text.as_str()).collect();
    assert!(
        texts
            .iter()
            .any(|t| t.contains("项目层有同名项") && t.contains("proj")),
        "{texts:?}"
    );
    let before_claude = tree(&f.user_home.join(".claude"));
    let before_ledger = fs::read(f.home.state_dir().join("adopted.toml")).unwrap();
    let applied = kernel.apply(&plan).unwrap();
    assert!(
        fs::symlink_metadata(f.user_home.join(".claude/skills/harness")).is_err(),
        "Claude Code 的那一处没了"
    );
    assert!(
        fs::symlink_metadata(f.user_home.join(".agents/skills/harness"))
            .unwrap()
            .file_type()
            .is_symlink(),
        "Codex 的那一处还在"
    );
    assert_eq!(tree(&objects), before_library, "资产库里的正本不动");
    let ledger = fs::read_to_string(f.home.state_dir().join("adopted.toml")).unwrap();
    let gone = format!(
        "\"{}\"",
        f.user_home.join(".claude/skills/harness").display()
    );
    assert!(!ledger.contains(&gone), "{ledger}");
    assert_eq!(ledger.matches("[[link]]").count(), 2, "{ledger}");

    let undo = kernel.plan_rollback_record(&applied.record).unwrap();
    kernel.apply(&undo).unwrap();
    assert_eq!(tree(&f.user_home.join(".claude")), before_claude);
    assert_eq!(
        fs::read(f.home.state_dir().join("adopted.toml")).unwrap(),
        before_ledger
    );
}

/// `fangcun add <来源>@<名字>`：清单加一行、锁、同步一份计划；再加一次是「已在清单里」。
#[test]
fn add_writes_manifest_lock_and_sync_in_one_plan() {
    let f = fixture("b6-add");
    let kernel = kernel(&f);
    lock_and_sync(&kernel, &f, &super::manifest("link", TDD_DECL, false));
    let before = fs::read_to_string(f.proj.join("fangcun.toml")).unwrap();

    let plan = kernel.plan_add(&f.proj, "acme/skills@review").unwrap();
    assert_eq!(plan.title, "把 review 加进 proj");
    let paths: Vec<String> = plan
        .changes
        .iter()
        .map(|c| c.path.to_string_lossy().into_owned())
        .collect();
    assert!(
        paths.iter().any(|p| p.ends_with("fangcun.toml")),
        "{paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.ends_with("fangcun.lock")),
        "{paths:?}"
    );
    assert!(
        paths.iter().any(|p| p.ends_with(".agents/skills/review")),
        "{paths:?}"
    );
    let applied = kernel.apply(&plan).unwrap();
    let after = fs::read_to_string(f.proj.join("fangcun.toml")).unwrap();
    let expected = before.replace(
        &format!("{TDD_DECL}\n"),
        &format!("{TDD_DECL}\nreview = {{ source = \"github:acme/skills\" }}\n"),
    );
    assert_eq!(after, expected, "原文不动，只在 [skills] 里加一行");
    assert!(f.proj.join(".agents/skills/review/SKILL.md").is_file());
    kernel.verify_frozen(&f.proj).unwrap();

    let again = kernel
        .plan_add(&f.proj, "github:acme/skills@review")
        .unwrap();
    assert_eq!(again.title, "review 已在 proj 的清单里");
    assert!(again.changes.is_empty());

    // 搜索资产库：review 已经存进来了，可以按名字加。
    let hits = kernel.search_assets("rev", false).unwrap();
    assert!(
        hits.iter()
            .any(|h| h.name == "review" && h.add.as_deref() == Some("review")),
        "{hits:?}"
    );

    // 回滚：清单、锁、链接都回去。
    let undo = kernel.plan_rollback_record(&applied.record).unwrap();
    kernel.apply(&undo).unwrap();
    assert_eq!(
        fs::read_to_string(f.proj.join("fangcun.toml")).unwrap(),
        before
    );
    assert!(!f.proj.join(".agents/skills/review").exists());
}
