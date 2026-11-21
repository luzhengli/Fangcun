//! MCP 在资产库里的样子（M1-20、9.3「MCP 的明文密钥」）。
//!
//! 正本是一个 TOML 文件，只有方寸管的三个字段 `command`、`args`、`env`，和清单里
//! `[mcp.<名称>]` 的写法一样。`env` 里的密钥一律是 `secret:<名称>` 引用：明文在入库之前
//! 换下来，由写入用例在应用时存进钥匙串；资产库里从来不放明文（规则 9）。
//!
//! 同样的三个字段总是写成同样的字节，所以内容哈希就是版本。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::content::hash_bytes;
use crate::model::ContentHash;
use crate::secrets::{SecretValue, looks_like_secret, split_env};
use crate::{Error, Result};

/// 一个 MCP 服务的启动配置，存进资产库的那一份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct McpSpec {
    pub command: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

impl McpSpec {
    /// 从工具配置里读到的一条服务。`env` 里看起来像明文密钥的值换成 `secret:` 引用，
    /// 引用的名字照登记时的规则（`secrets::reference_name`）。返回引用版本和换下来的明文。
    pub fn from_config(
        server: &str,
        command: &str,
        args: &[String],
        env: &[(String, String)],
    ) -> (Self, Vec<(String, SecretValue)>) {
        let (env, plain) = split_env(server, env.iter().map(|(k, v)| (k, v)));
        (
            Self {
                command: command.to_owned(),
                args: args.to_vec(),
                env,
            },
            plain,
        )
    }

    /// 存进资产库的字节：字段顺序固定，`env` 按变量名排序。
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        toml::to_string(self)
            .map(String::into_bytes)
            .map_err(|e| Error::Parse {
                origin: "MCP 正本".to_owned(),
                message: e.to_string(),
            })
    }

    pub fn parse(origin: &str, bytes: &[u8]) -> Result<Self> {
        let text = std::str::from_utf8(bytes).map_err(|e| Error::Parse {
            origin: origin.to_owned(),
            message: e.to_string(),
        })?;
        toml::from_str(text).map_err(|e| Error::Parse {
            origin: origin.to_owned(),
            message: e.to_string(),
        })
    }

    /// 内容哈希，也就是资产库里的版本。
    pub fn hash(&self) -> Result<ContentHash> {
        Ok(hash_bytes(&self.to_bytes()?))
    }

    /// 还留着明文的变量。入库前必须为空。
    pub fn plaintext_vars(&self) -> Vec<&str> {
        self.env
            .iter()
            .filter(|(k, v)| looks_like_secret(k, v))
            .map(|(k, _)| k.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_is_swapped_for_references_and_bytes_are_stable() {
        let env = vec![
            ("GITHUB_TOKEN".to_owned(), "ghp_abcdefghijklmnop".to_owned()),
            ("LOG_LEVEL".to_owned(), "debug".to_owned()),
        ];
        let (spec, plain) =
            McpSpec::from_config("github", "npx", &["-y".to_owned(), "gh".to_owned()], &env);
        assert_eq!(spec.env["GITHUB_TOKEN"], "secret:github-token");
        assert_eq!(spec.env["LOG_LEVEL"], "debug");
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].0, "github-token");
        assert_eq!(plain[0].1.expose(), "ghp_abcdefghijklmnop");
        assert!(spec.plaintext_vars().is_empty());

        let bytes = spec.to_bytes().unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(!text.contains("ghp_"), "{text}");
        assert_eq!(McpSpec::parse("t", &bytes).unwrap(), spec);
        // 变量顺序不同，字节一样。
        let reversed: Vec<(String, String)> = env.iter().rev().cloned().collect();
        let (again, _) = McpSpec::from_config(
            "github",
            "npx",
            &["-y".to_owned(), "gh".to_owned()],
            &reversed,
        );
        assert_eq!(again.hash().unwrap(), spec.hash().unwrap());
    }
}
