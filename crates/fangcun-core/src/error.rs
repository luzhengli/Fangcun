//! 内核统一的错误类型。

use std::path::PathBuf;

use serde::Serialize;
use ts_rs::TS;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// 骨架阶段的占位。参数是「模块::函数」，实现后删掉对应的返回。
    #[error("尚未实现：{0}")]
    NotImplemented(&'static str),

    /// 内核以只读方式打开时调用了会写入的用例。M0 全程只读。
    #[error("当前是只读模式，不能执行：{0}")]
    ReadOnly(&'static str),

    #[error("不认识的工具：{0}")]
    UnknownTool(String),

    #[error("没有找到：{0}")]
    NotFound(String),

    #[error("找不到用户主目录")]
    NoHomeDir,

    #[error("读写 {path} 失败")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("解析 {origin} 失败：{message}")]
    Parse { origin: String, message: String },
}

impl Error {
    /// 给界面和命令行用的稳定错误码，不随文案变化。
    pub fn code(&self) -> &'static str {
        match self {
            Error::NotImplemented(_) => "not-implemented",
            Error::ReadOnly(_) => "read-only",
            Error::UnknownTool(_) => "unknown-tool",
            Error::NotFound(_) => "not-found",
            Error::NoHomeDir => "no-home-dir",
            Error::Io { .. } => "io",
            Error::Parse { .. } => "parse",
        }
    }
}

/// 错误过 IPC 时的样子。界面按 `code` 分支，`message` 原样展示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

impl ErrorPayload {
    /// 不属于内核的故障，例如外壳的线程出错。
    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: "internal".to_owned(),
            message: message.into(),
        }
    }
}

impl From<&Error> for ErrorPayload {
    fn from(error: &Error) -> Self {
        Self {
            code: error.code().to_owned(),
            message: error.to_string(),
        }
    }
}
