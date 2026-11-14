//! 密钥（F6、7.5）：MCP 密钥存进 macOS 钥匙串。M1 实现。
//!
//! - 提交进仓库的配置只含引用 `secret:<名称>`，明文只落在不提交的本地配置里。
//! - 存入、替换、删除各记一条活动，只记名称和位置，从不记值，不提供回滚。
//! - 密钥的值只在本模块和物化器写本地配置的那一步出现，不进日志、不过 IPC、不进快照。

use std::fmt;

use crate::Result;

/// 清单里引用密钥的前缀，例如 `secret:github-token`。
pub const SECRET_PREFIX: &str = "secret:";

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

/// 密钥的存放处。正式实现是 macOS 钥匙串，测试用内存实现。
pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Result<Option<SecretValue>>;
    fn set(&self, name: &str, value: &SecretValue) -> Result<()>;
    fn delete(&self, name: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_prints_the_value() {
        let secret = SecretValue::new("ghp_example");
        assert!(!format!("{secret:?}").contains("ghp_example"));
    }
}
