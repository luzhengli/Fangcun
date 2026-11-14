//! 监听器（F4、7.2）：监听已登记项目与全局目录，发现漂移。M1 实现。
//!
//! 监听器只发现、只记录，不改任何受管文件。验收：Agent 改了 skill，五秒内出现漂移提示。
//! 只在桌面应用里常驻；命令行每次运行时直接比对，不启动监听。

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::ProjectId;
use crate::{Error, Result};

/// 内核主动推给外壳的事件。桌面外壳把它转发给界面，命令行不订阅。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case", tag = "type")]
#[ts(export)]
pub enum KernelEvent {
    /// 受管副本被修改。
    DriftDetected { project: ProjectId, asset: String },
    /// 活动记录有新增。界面据此刷新待处理数和活动页。
    JournalChanged,
}

/// 事件的接收方。外壳实现它。
pub trait EventSink: Send + Sync {
    fn emit(&self, event: KernelEvent);
}

#[derive(Debug)]
pub struct Watcher {}

impl Watcher {
    /// 开始监听。`roots` 是已登记的项目目录，全局目录从适配器查。
    pub fn start(_roots: Vec<PathBuf>, _sink: Arc<dyn EventSink>) -> Result<Self> {
        Err(Error::NotImplemented("watcher::Watcher::start"))
    }
}
