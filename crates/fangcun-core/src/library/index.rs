//! 资产库的索引 `~/.fangcun/index.sqlite`（7.2）：每项资产的来源、各个版本、含不含脚本。
//!
//! 同名、同类、同来源算同一项资产；来源未知时 `source` 存空字符串（SQLite 的唯一约束不把
//! `NULL` 当作相等）。版本按哈希去重，`seq` 记先后。写入用 `BEGIN IMMEDIATE`，
//! 桌面应用和命令行同时收编时由 SQLite 排队，`busy_timeout` 5 秒。

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};

use crate::model::{AssetKind, ContentHash, SourceRef, Timestamp, Version};
use crate::{Error, Result};

/// 表结构的版本，存在 `PRAGMA user_version`。改表结构时加一，并在 `migrate` 里补升级。
const SCHEMA: i32 = 1;

pub(super) struct IndexedAsset {
    pub name: String,
    pub kind: AssetKind,
    pub source: Option<SourceRef>,
    pub has_scripts: bool,
    pub versions: Vec<Version>,
}

/// 读全部资产。索引文件不存在时是空的，也不创建它。
pub(super) fn read(file: &Path) -> Result<Vec<IndexedAsset>> {
    if !file.exists() {
        return Ok(Vec::new());
    }
    let conn =
        Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(db(file))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(db(file))?;
    let mut assets = Vec::new();
    let mut stmt = conn
        .prepare("SELECT id, name, kind, source, has_scripts FROM assets")
        .map_err(db(file))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })
        .map_err(db(file))?;
    let mut versions = conn
        .prepare("SELECT hash, commit_id, tag FROM versions WHERE asset_id = ?1 ORDER BY seq")
        .map_err(db(file))?;
    for row in rows {
        let (id, name, kind, source, has_scripts) = row.map_err(db(file))?;
        let list = versions
            .query_map([id], |row| {
                Ok(Version {
                    hash: ContentHash(row.get(0)?),
                    commit: row.get(1)?,
                    tag: row.get(2)?,
                })
            })
            .map_err(db(file))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(db(file))?;
        assets.push(IndexedAsset {
            name,
            kind: parse_kind(&kind, file)?,
            source: (!source.is_empty()).then_some(SourceRef(source)),
            has_scripts,
            versions: list,
        });
    }
    Ok(assets)
}

/// 记一个版本：资产没有就建，版本已有就不重复记。
pub(super) fn record(
    file: &Path,
    name: &str,
    kind: AssetKind,
    source: Option<&SourceRef>,
    has_scripts: bool,
    version: &Version,
) -> Result<()> {
    let mut conn = open_rw(file)?;
    let tx = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db(file))?;
    let source = source.map_or("", |s| s.0.as_str());
    tx.execute(
        "INSERT INTO assets (name, kind, source, has_scripts) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (name, kind, source) DO UPDATE SET has_scripts = has_scripts OR excluded.has_scripts",
        params![name, kind_str(kind), source, has_scripts],
    )
    .map_err(db(file))?;
    let id: i64 = tx
        .query_row(
            "SELECT id FROM assets WHERE name = ?1 AND kind = ?2 AND source = ?3",
            params![name, kind_str(kind), source],
            |row| row.get(0),
        )
        .map_err(db(file))?;
    let known: Option<i64> = tx
        .query_row(
            "SELECT seq FROM versions WHERE asset_id = ?1 AND hash = ?2",
            params![id, version.hash.0],
            |row| row.get(0),
        )
        .optional()
        .map_err(db(file))?;
    if known.is_none() {
        tx.execute(
            "INSERT INTO versions (asset_id, hash, commit_id, tag, added) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, version.hash.0, version.commit, version.tag, Timestamp::now().0],
        )
        .map_err(db(file))?;
    }
    tx.commit().map_err(db(file))
}

fn open_rw(file: &Path) -> Result<Connection> {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let conn = Connection::open(file).map_err(db(file))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(db(file))?;
    migrate(&conn, file)?;
    Ok(conn)
}

fn migrate(conn: &Connection, file: &Path) -> Result<()> {
    let version: i32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(db(file))?;
    if version >= SCHEMA {
        return Ok(());
    }
    conn.execute_batch(
        "BEGIN IMMEDIATE;
         CREATE TABLE IF NOT EXISTS assets (
             id          INTEGER PRIMARY KEY,
             name        TEXT NOT NULL,
             kind        TEXT NOT NULL,
             source      TEXT NOT NULL DEFAULT '',
             has_scripts INTEGER NOT NULL DEFAULT 0,
             UNIQUE (name, kind, source)
         );
         CREATE TABLE IF NOT EXISTS versions (
             seq       INTEGER PRIMARY KEY AUTOINCREMENT,
             asset_id  INTEGER NOT NULL REFERENCES assets (id),
             hash      TEXT NOT NULL,
             commit_id TEXT,
             tag       TEXT,
             added     TEXT NOT NULL,
             UNIQUE (asset_id, hash)
         );
         PRAGMA user_version = 1;
         COMMIT;",
    )
    .map_err(db(file))
}

fn kind_str(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Skill => "skill",
        AssetKind::Mcp => "mcp",
        AssetKind::Instruction => "instruction",
    }
}

fn parse_kind(text: &str, file: &Path) -> Result<AssetKind> {
    match text {
        "skill" => Ok(AssetKind::Skill),
        "mcp" => Ok(AssetKind::Mcp),
        "instruction" => Ok(AssetKind::Instruction),
        other => Err(Error::Parse {
            origin: file.display().to_string(),
            message: format!("不认识的资产类型 {other}"),
        }),
    }
}

fn db(file: &Path) -> impl FnOnce(rusqlite::Error) -> Error {
    let path = file.to_path_buf();
    move |e| Error::Database {
        path,
        message: e.to_string(),
    }
}
