//! 密钥（F6、7.5、M1-17）：MCP 密钥存进 macOS 钥匙串。
//!
//! - 清单里写引用 `secret:<名称>`。同步时提交进仓库的配置只写环境变量引用（各工具的写法见适配器
//!   `[mcp-fields]` 的 `env-reference`、`env-forward`），明文只落在工具的个人层（9.3）。
//! - 钥匙串条目的服务名是 `fangcun`，账户名是密钥名称。
//! - 存入、替换、删除各记一条活动，只记名称和位置，从不记值，不提供回滚（9.3）。
//! - 密钥的值只在本模块和物化器写个人层配置的那一步出现，不进日志、不过 IPC、不进快照：
//!   快照里方寸写的明文换成 [`marker`]，回滚时再从钥匙串取回（见 `materializer::snapshot`）。

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

use crate::{Error, Result};

/// 清单里引用密钥的前缀，例如 `secret:github-token`。
pub const SECRET_PREFIX: &str = "secret:";

/// 钥匙串条目的服务名。
pub const KEYCHAIN_SERVICE: &str = "fangcun";

/// 密钥的值。`Debug` 不输出内容，防止被打进日志或错误信息。
#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// 取出明文。只应在写入本地配置的那一步调用。
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretValue(***)")
    }
}

/// `secret:<名称>` 里的名称。不是引用时为空。
pub fn reference(value: &str) -> Option<&str> {
    value.strip_prefix(SECRET_PREFIX)
}

/// 名称只许字母、数字、`.`、`_`、`-`，1 到 64 个字符。
pub fn check_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if ok {
        Ok(())
    } else {
        Err(Error::Secret(format!(
            "密钥名称「{name}」不合规：只能用字母、数字、.、_、-，最多 64 个字符"
        )))
    }
}

/// 快照里代替明文的占位，回滚时换回钥匙串里的值。
pub fn marker(name: &str) -> String {
    format!("«fangcun-secret:{name}»")
}

/// 密钥的存放处。正式实现是 macOS 钥匙串，测试用 [`MemoryStore`]。
pub trait SecretStore: Send + Sync + fmt::Debug {
    fn get(&self, name: &str) -> Result<Option<SecretValue>>;
    fn set(&self, name: &str, value: &SecretValue) -> Result<()>;
    /// 删除。本来就没有时返回 `false`。
    fn delete(&self, name: &str) -> Result<bool>;
    /// 存着的全部名称，不读值。
    fn names(&self) -> Result<Vec<String>>;
}

/// 本机默认的存放处：macOS 是钥匙串，别的平台还不支持。
pub fn default_store() -> std::sync::Arc<dyn SecretStore> {
    #[cfg(target_os = "macos")]
    {
        std::sync::Arc::new(Keychain)
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::sync::Arc::new(Unsupported)
    }
}

/// macOS 钥匙串里服务名为 `fangcun` 的通用密码。
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Keychain;

#[cfg(target_os = "macos")]
mod keychain {
    use security_framework::base::Error as SfError;
    use security_framework::item::{ItemClass, ItemSearchOptions, Limit};
    use security_framework::passwords;

    use super::{KEYCHAIN_SERVICE, Keychain, SecretStore, SecretValue};
    use crate::{Error, Result};

    /// errSecItemNotFound
    const NOT_FOUND: i32 = -25300;

    fn fail(action: &str, name: &str, e: &SfError) -> Error {
        Error::Secret(format!(
            "钥匙串{action} {name} 失败（{}）",
            e.message()
                .unwrap_or_else(|| format!("OSStatus {}", e.code()))
        ))
    }

    impl SecretStore for Keychain {
        fn get(&self, name: &str) -> Result<Option<SecretValue>> {
            match passwords::get_generic_password(KEYCHAIN_SERVICE, name) {
                Ok(bytes) => String::from_utf8(bytes)
                    .map(|s| Some(SecretValue::new(s)))
                    .map_err(|_| Error::Secret(format!("钥匙串里的 {name} 不是文本"))),
                Err(e) if e.code() == NOT_FOUND => Ok(None),
                Err(e) => Err(fail("读取", name, &e)),
            }
        }

        fn set(&self, name: &str, value: &SecretValue) -> Result<()> {
            passwords::set_generic_password(KEYCHAIN_SERVICE, name, value.expose().as_bytes())
                .map_err(|e| fail("写入", name, &e))
        }

        fn delete(&self, name: &str) -> Result<bool> {
            match passwords::delete_generic_password(KEYCHAIN_SERVICE, name) {
                Ok(()) => Ok(true),
                Err(e) if e.code() == NOT_FOUND => Ok(false),
                Err(e) => Err(fail("删除", name, &e)),
            }
        }

        fn names(&self) -> Result<Vec<String>> {
            let found = ItemSearchOptions::new()
                .class(ItemClass::generic_password())
                .service(KEYCHAIN_SERVICE)
                .load_attributes(true)
                .limit(Limit::All)
                .search();
            let results = match found {
                Ok(results) => results,
                Err(e) if e.code() == NOT_FOUND => return Ok(Vec::new()),
                Err(e) => return Err(fail("列出", KEYCHAIN_SERVICE, &e)),
            };
            let mut names: Vec<String> = results
                .iter()
                .filter_map(|r| r.simplify_dict())
                .filter_map(|d| d.get("acct").cloned())
                .collect();
            names.sort();
            names.dedup();
            Ok(names)
        }
    }
}

/// 还没有钥匙串支持的平台：一律报错，不静默失败（7.3）。
#[cfg(not(target_os = "macos"))]
#[derive(Debug, Clone, Copy, Default)]
pub struct Unsupported;

#[cfg(not(target_os = "macos"))]
impl SecretStore for Unsupported {
    fn get(&self, _name: &str) -> Result<Option<SecretValue>> {
        Err(Error::Secret("这个平台还没有钥匙串支持".to_owned()))
    }
    fn set(&self, _name: &str, _value: &SecretValue) -> Result<()> {
        Err(Error::Secret("这个平台还没有钥匙串支持".to_owned()))
    }
    fn delete(&self, _name: &str) -> Result<bool> {
        Err(Error::Secret("这个平台还没有钥匙串支持".to_owned()))
    }
    fn names(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

/// 内存里的存放处。测试用，进程结束就没了。
#[derive(Default)]
pub struct MemoryStore(Mutex<BTreeMap<String, SecretValue>>);

impl fmt::Debug for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MemoryStore")
    }
}

impl MemoryStore {
    pub fn with(entries: &[(&str, &str)]) -> Self {
        Self(Mutex::new(
            entries
                .iter()
                .map(|(k, v)| ((*k).to_owned(), SecretValue::new(*v)))
                .collect(),
        ))
    }

    fn map(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, SecretValue>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Result<Option<SecretValue>> {
        Ok(self.map().get(name).cloned())
    }
    fn set(&self, name: &str, value: &SecretValue) -> Result<()> {
        self.map().insert(name.to_owned(), value.clone());
        Ok(())
    }
    fn delete(&self, name: &str) -> Result<bool> {
        Ok(self.map().remove(name).is_some())
    }
    fn names(&self) -> Result<Vec<String>> {
        Ok(self.map().keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_value() {
        let secret = SecretValue::new("ghp_example");
        assert!(!format!("{secret:?}").contains("ghp_example"));
    }

    #[test]
    fn names_are_checked() {
        assert!(check_name("github-token").is_ok());
        assert!(check_name("a.b_c-1").is_ok());
        assert!(check_name("").is_err());
        assert!(check_name("有空格 的").is_err());
        assert!(check_name("../x").is_err());
    }

    #[test]
    fn references_are_parsed() {
        assert_eq!(reference("secret:github-token"), Some("github-token"));
        assert_eq!(reference("${GITHUB_TOKEN}"), None);
    }

    #[test]
    fn memory_store_round_trips() {
        let store = MemoryStore::default();
        store.set("a", &SecretValue::new("1")).unwrap();
        assert_eq!(store.get("a").unwrap().unwrap().expose(), "1");
        assert_eq!(store.names().unwrap(), vec!["a".to_owned()]);
        assert!(store.delete("a").unwrap());
        assert!(!store.delete("a").unwrap());
    }
}
