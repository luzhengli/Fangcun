//! 与 `skills-lock.json` 互通（F3、7.4、9.2）。
//!
//! `skills` 命令行把装进项目的 skill 记在项目根目录的 `skills-lock.json`，装到
//! `.agents/skills/<名称>`。格式照 `skills` 1.9.0：
//!
//! ```json
//! { "version": 2, "skills": { "tdd": {
//!     "source": "obra/superpowers", "sourceType": "github",
//!     "ref": "main", "commit": "b7e9…", "skillPath": "skills/tdd/SKILL.md",
//!     "computedHash": "9f2c…" } } }
//! ```
//!
//! `version` 1 的哈希是旧算法，读的时候和 CLI 一样：对已安装的目录按旧算法校验，
//! 对得上就换算成新算法（v2，与锁同算法），对不上就当没有哈希。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::library::content::{hash_dir, legacy_hash_dir};
use crate::model::ContentHash;
use crate::{Error, Result};

/// 读出来、换算过哈希的一份 `skills-lock.json`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsLock {
    pub path: PathBuf,
    /// 文件里写的版本，1 或 2。
    pub version: u32,
    pub skills: BTreeMap<String, SkillsLockEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsLockEntry {
    /// GitHub 来源是 `owner/repo`。
    pub source: String,
    /// `github`、`local`、`git`、`well-known`、`node_modules` 等。
    pub source_type: String,
    /// 装的时候指定的分支或标签。
    pub git_ref: Option<String>,
    pub commit: Option<String>,
    /// 仓库里 `SKILL.md` 所在的目录，从 `skillPath` 去掉文件名。
    pub path: Option<String>,
    /// v2 算法的内容哈希。v1 的锁校验不过时为空。
    pub hash: Option<ContentHash>,
}

impl SkillsLockEntry {
    /// 能写成 `github:owner/repo` 的来源。
    pub fn github_source(&self) -> Option<String> {
        if self.source_type != "github" {
            return None;
        }
        let mut parts = self.source.split('/');
        let (Some(owner), Some(repo), None) = (parts.next(), parts.next(), parts.next()) else {
            return None;
        };
        (!owner.is_empty() && !repo.is_empty()).then(|| format!("github:{owner}/{repo}"))
    }
}

#[derive(Deserialize)]
struct RawLock {
    version: u32,
    skills: BTreeMap<String, RawEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawEntry {
    #[serde(default)]
    source: String,
    #[serde(default)]
    source_type: String,
    #[serde(default, rename = "ref")]
    git_ref: Option<String>,
    #[serde(default)]
    commit: Option<String>,
    #[serde(default)]
    skill_path: Option<String>,
    #[serde(default)]
    computed_hash: Option<String>,
}

/// 读项目里的一份 `skills-lock.json`。`installed` 是 skill 装在哪个目录（`.agents/skills`），
/// v1 的哈希按 `installed/<名称>` 校验。文件不存在时是 `None`。
pub fn read_skills_lock(path: &Path, installed: &Path) -> Result<Option<SkillsLock>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(Error::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let raw: RawLock = serde_json::from_str(&text).map_err(|e| Error::Parse {
        origin: path.display().to_string(),
        message: e.to_string(),
    })?;
    if raw.version != 1 && raw.version != 2 {
        return Err(Error::Parse {
            origin: path.display().to_string(),
            message: format!("不认识的版本 {}，只认 1 和 2", raw.version),
        });
    }
    let skills = raw
        .skills
        .into_iter()
        .map(|(name, entry)| {
            let recorded = entry.computed_hash.filter(|h| !h.is_empty());
            let hash = if raw.version == 1 {
                recorded.and_then(|v1| migrate_hash(&installed.join(&name), &v1))
            } else {
                recorded.map(ContentHash)
            };
            let path = entry.skill_path.map(|p| {
                p.strip_suffix("SKILL.md")
                    .unwrap_or(&p)
                    .trim_matches('/')
                    .to_owned()
            });
            let entry = SkillsLockEntry {
                source: entry.source,
                source_type: entry.source_type,
                git_ref: entry.git_ref.filter(|r| !r.is_empty()),
                commit: entry.commit.filter(|c| !c.is_empty()),
                path,
                hash,
            };
            (name, entry)
        })
        .collect();
    Ok(Some(SkillsLock {
        path: path.to_path_buf(),
        version: raw.version,
        skills,
    }))
}

/// v1 → v2：已安装的目录按旧算法算出来和锁里一样，才换算成新算法的哈希。
fn migrate_hash(dir: &Path, v1: &str) -> Option<ContentHash> {
    if !dir.join("SKILL.md").is_file() {
        return None;
    }
    (legacy_hash_dir(dir).0 == v1).then(|| hash_dir(dir))
}

/// [`export_skills_lock`] 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedSkillsLock {
    /// 新的 `skills-lock.json` 全文。
    pub text: String,
    /// 导出的项。
    pub exported: Vec<String>,
    /// 锁里有、但不是 GitHub 来源没有导出的项。
    pub skipped: Vec<String>,
    /// 原文件里方寸不管、原样留下的项。
    pub kept: Vec<String>,
}

/// 把锁里 GitHub 来源的 skill 导出成 `skills-lock.json`（版本 2，`fangcun export skills-lock`）。
///
/// 每项照 `skills` 1.9.0 写项目锁的字段与顺序：`source`、`ref`、`commit`、`sourceType`、
/// `skillPath`、`computedHash`。`existing` 是项目里原有的那份：方寸锁里没有的项原样留下
/// （版本 1 的哈希先换算，校验不过的置空，和 CLI 读旧锁时一样）。
pub fn export_skills_lock(
    lock: &super::Lock,
    existing: &Path,
    installed: &Path,
) -> Result<ExportedSkillsLock> {
    let mut entries: BTreeMap<String, String> = BTreeMap::new();
    let mut out = ExportedSkillsLock {
        text: String::new(),
        exported: Vec::new(),
        skipped: Vec::new(),
        kept: Vec::new(),
    };

    for (name, item) in &lock.skills {
        let Some(repo) = item.source.strip_prefix("github:") else {
            out.skipped.push(name.clone());
            continue;
        };
        let mut fields = vec![("source", repo.to_owned())];
        if let Some(r) = item.track.as_ref().or(item.pin.as_ref()) {
            fields.push(("ref", r.clone()));
        }
        if let Some(commit) = &item.commit {
            fields.push(("commit", commit.clone()));
        }
        fields.push(("sourceType", "github".to_owned()));
        let skill_path = if item.path.is_empty() {
            "SKILL.md".to_owned()
        } else {
            format!("{}/SKILL.md", item.path)
        };
        fields.push(("skillPath", skill_path));
        fields.push(("computedHash", item.hash.0.clone()));
        let body: Vec<String> = fields
            .iter()
            .map(|(k, v)| format!("      {}: {}", json_string(k), json_string(v)))
            .collect();
        entries.insert(name.clone(), format!("{{\n{}\n    }}", body.join(",\n")));
        out.exported.push(name.clone());
    }

    if let Some(previous) = read_skills_lock(existing, installed)? {
        let text = std::fs::read_to_string(existing).map_err(|source| Error::Io {
            path: existing.to_path_buf(),
            source,
        })?;
        let raw: serde_json::Value = serde_json::from_str(&text).map_err(|e| Error::Parse {
            origin: existing.display().to_string(),
            message: e.to_string(),
        })?;
        for (name, value) in raw
            .get("skills")
            .and_then(|s| s.as_object())
            .into_iter()
            .flatten()
        {
            if entries.contains_key(name) || out.skipped.contains(name) {
                continue;
            }
            let mut value = value.clone();
            if previous.version == 1
                && let Some(object) = value.as_object_mut()
            {
                let hash = previous
                    .skills
                    .get(name)
                    .and_then(|e| e.hash.as_ref())
                    .map(|h| h.0.clone())
                    .unwrap_or_default();
                object.insert("computedHash".to_owned(), serde_json::Value::String(hash));
            }
            let pretty = serde_json::to_string_pretty(&value).map_err(|e| Error::Parse {
                origin: existing.display().to_string(),
                message: e.to_string(),
            })?;
            entries.insert(name.clone(), pretty.replace('\n', "\n    "));
            out.kept.push(name.clone());
        }
    }

    let skills: Vec<String> = entries
        .iter()
        .map(|(name, body)| format!("    {}: {body}", json_string(name)))
        .collect();
    out.text = if skills.is_empty() {
        "{\n  \"version\": 2,\n  \"skills\": {}\n}\n".to_owned()
    } else {
        format!(
            "{{\n  \"version\": 2,\n  \"skills\": {{\n{}\n  }}\n}}\n",
            skills.join(",\n")
        )
    };
    Ok(out)
}

fn json_string(value: &str) -> String {
    serde_json::Value::String(value.to_owned()).to_string()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fangcun-interop-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn v1_hashes_are_verified_then_converted() {
        let root = temp("v1");
        let installed = root.join(".agents/skills");
        for name in ["kept", "edited"] {
            let dir = installed.join(name);
            std::fs::create_dir_all(dir.join("scripts")).unwrap();
            std::fs::write(dir.join("SKILL.md"), format!("---\nname: {name}\n---\n")).unwrap();
            std::fs::write(dir.join("scripts/main.ts"), "x").unwrap();
        }
        let kept_v1 = legacy_hash_dir(&installed.join("kept")).0;
        let edited_v1 = legacy_hash_dir(&installed.join("edited")).0;
        std::fs::write(installed.join("edited/SKILL.md"), "changed").unwrap();
        let lock = root.join("skills-lock.json");
        std::fs::write(
            &lock,
            format!(
                r#"{{ "version": 1, "skills": {{
                    "kept": {{ "source": "acme/skills", "sourceType": "github", "computedHash": "{kept_v1}" }},
                    "edited": {{ "source": "acme/skills", "sourceType": "github", "computedHash": "{edited_v1}" }},
                    "gone": {{ "source": "acme/skills", "sourceType": "github", "computedHash": "00" }}
                }} }}"#
            ),
        )
        .unwrap();

        let read = read_skills_lock(&lock, &installed).unwrap().unwrap();
        assert_eq!(read.version, 1);
        assert_eq!(
            read.skills["kept"].hash,
            Some(hash_dir(&installed.join("kept")))
        );
        assert_eq!(read.skills["edited"].hash, None);
        assert_eq!(read.skills["gone"].hash, None);
        assert_eq!(
            read.skills["kept"].github_source().as_deref(),
            Some("github:acme/skills")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn export_writes_github_items_and_keeps_the_rest() {
        use crate::manifest::{Lock, LockedSkill};

        let root = temp("export");
        let installed = root.join(".agents/skills");
        std::fs::create_dir_all(installed.join("theirs")).unwrap();
        std::fs::write(
            installed.join("theirs/SKILL.md"),
            "---\nname: theirs\n---\n",
        )
        .unwrap();
        let theirs_v1 = legacy_hash_dir(&installed.join("theirs")).0;
        let existing = root.join("skills-lock.json");
        std::fs::write(
            &existing,
            format!(
                r#"{{ "version": 1, "skills": {{
                    "tdd": {{ "source": "old/old", "sourceType": "github", "computedHash": "x" }},
                    "theirs": {{ "source": "other/repo", "sourceType": "github", "computedHash": "{theirs_v1}" }}
                }} }}"#
            ),
        )
        .unwrap();
        let mut lock = Lock::default();
        let locked = |source: &str, path: &str| LockedSkill {
            source: source.to_owned(),
            track: Some("main".to_owned()),
            pin: None,
            tag: None,
            commit: Some("abc".to_owned()),
            path: path.to_owned(),
            hash: ContentHash("9f2c".to_owned()),
            bundle: None,
        };
        lock.skills.insert(
            "tdd".to_owned(),
            locked("github:obra/superpowers", "skills/tdd"),
        );
        lock.skills.insert(
            "mine".to_owned(),
            locked("local:luyao-skills", "skills/mine"),
        );

        let out = export_skills_lock(&lock, &existing, &installed).unwrap();
        assert_eq!(out.exported, ["tdd"]);
        assert_eq!(out.skipped, ["mine"]);
        assert_eq!(out.kept, ["theirs"]);
        let theirs_v2 = hash_dir(&installed.join("theirs")).0;
        assert_eq!(
            out.text,
            format!(
                r#"{{
  "version": 2,
  "skills": {{
    "tdd": {{
      "source": "obra/superpowers",
      "ref": "main",
      "commit": "abc",
      "sourceType": "github",
      "skillPath": "skills/tdd/SKILL.md",
      "computedHash": "9f2c"
    }},
    "theirs": {{
      "computedHash": "{theirs_v2}",
      "source": "other/repo",
      "sourceType": "github"
    }}
  }}
}}
"#
            )
        );
        // 导出的再读回来，和锁一致。
        std::fs::write(&existing, &out.text).unwrap();
        let read = read_skills_lock(&existing, &installed).unwrap().unwrap();
        let tdd = &read.skills["tdd"];
        assert_eq!(tdd.path.as_deref(), Some("skills/tdd"));
        assert_eq!(tdd.commit.as_deref(), Some("abc"));
        assert_eq!(tdd.hash, Some(ContentHash("9f2c".to_owned())));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn v2_entries_keep_ref_commit_and_path() {
        let root = temp("v2");
        let lock = root.join("skills-lock.json");
        std::fs::write(
            &lock,
            r#"{ "version": 2, "skills": {
                "tdd": { "source": "obra/superpowers", "sourceType": "github", "ref": "main",
                         "commit": "abc123", "skillPath": "skills/tdd/SKILL.md", "computedHash": "9f2c" },
                "mine": { "source": "../my-skills", "sourceType": "local", "computedHash": "" }
            } }"#,
        )
        .unwrap();
        let read = read_skills_lock(&lock, &root).unwrap().unwrap();
        let tdd = &read.skills["tdd"];
        assert_eq!(tdd.git_ref.as_deref(), Some("main"));
        assert_eq!(tdd.commit.as_deref(), Some("abc123"));
        assert_eq!(tdd.path.as_deref(), Some("skills/tdd"));
        assert_eq!(tdd.hash, Some(ContentHash("9f2c".to_owned())));
        assert_eq!(read.skills["mine"].github_source(), None);
        assert_eq!(read.skills["mine"].hash, None);
        assert!(
            read_skills_lock(&root.join("none.json"), &root)
                .unwrap()
                .is_none()
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
