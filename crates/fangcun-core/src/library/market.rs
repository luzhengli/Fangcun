//! 公开市场（F2、M1-pre-08）：skills.sh，只用来搜索和发现。
//!
//! 用的是 `skills` 命令行也在用的接口 `GET https://skills.sh/api/search?q=<词>`。
//! 只在人主动搜索时请求，只发搜索词。搜到的项落成 `github:<owner>/<repo>` 来源，
//! 之后照 git 来源解析（[`super::source`]）。
//!
//! 请求用本机的 `curl`，不为一个请求引入 HTTP 库。

use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::model::SourceRef;
use crate::{Error, Result};

const SEARCH_URL: &str = "https://skills.sh/api/search";

/// 一条搜索结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketHit {
    /// skill 的名字，也是清单里的键。
    pub name: String,
    pub source: SourceRef,
    /// skills.sh 统计的安装次数，用来排序。
    pub installs: u64,
}

pub fn search(query: &str) -> Result<Vec<MarketHit>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let output = Command::new("curl")
        .args(["-fsS", "--max-time", "15", "-G", "--data-urlencode"])
        .arg(format!("q={query}"))
        .arg("--")
        .arg(SEARCH_URL)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| Error::Source(format!("运行 curl 失败：{e}")))?;
    if !output.status.success() {
        return Err(Error::Source(format!(
            "搜索 skills.sh 失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse(&output.stdout)
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    skills: Vec<RawHit>,
}

#[derive(Deserialize)]
struct RawHit {
    name: String,
    source: Option<String>,
    #[serde(default)]
    installs: u64,
}

/// 解析接口的返回。来源不是 `owner/repo` 的结果（例如其他站点的 well-known 来源）丢掉，
/// 方寸只认 `github:`。
pub(crate) fn parse(body: &[u8]) -> Result<Vec<MarketHit>> {
    let response: Response = serde_json::from_slice(body).map_err(|e| Error::Parse {
        origin: "skills.sh 的搜索结果".to_owned(),
        message: e.to_string(),
    })?;
    Ok(response
        .skills
        .into_iter()
        .filter_map(|hit| {
            let source = hit.source?;
            let (owner, repo) = source.split_once('/')?;
            let plain = |s: &str| {
                !s.is_empty()
                    && !s.starts_with(['.', '-'])
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            };
            (plain(owner) && plain(repo)).then(|| MarketHit {
                name: hit.name,
                source: SourceRef(format!("github:{owner}/{repo}")),
                installs: hit.installs,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_github_hits_and_maps_them_to_sources() {
        // 2026-11-19 实际返回的样子，删掉了计时字段。
        let body = br#"{"query":"tdd","searchType":"fuzzy","skills":[
            {"id":"mattpocock/skills/tdd","source":"mattpocock/skills","skillId":"tdd","name":"tdd","installs":1255353},
            {"id":"x","source":"example.com","skillId":"y","name":"y","installs":3},
            {"id":"z","source":"../evil/repo","skillId":"z","name":"z","installs":1}
        ],"count":3}"#;
        let hits = parse(body).unwrap();
        assert_eq!(
            hits,
            [MarketHit {
                name: "tdd".to_owned(),
                source: SourceRef("github:mattpocock/skills".to_owned()),
                installs: 1_255_353,
            }]
        );
    }
}
