//! 物化器（F4、7.2）：计划、应用、快照、回滚，并记录所有权。M1 实现。
//!
//! 这是内核里唯一允许改动「方寸目录以外」文件的模块。规则：
//!
//! 1. 预览先于写入。任何落盘动作先得到一份 [`ChangePlan`]，确认后才 [`apply`]。
//! 2. 一切可撤销。每次 `apply` 之前留快照，受影响的每个范围各留一个。
//! 3. 只动自己的东西。写入前查所有权；共享的 JSON 与 TOML 按键合并，不整文件覆盖。
//! 4. 写入范围限定在已登记项目与适配器声明的工具配置目录（7.5）。
//! 5. 每次写入记一条到活动记录。
//!
//! 实现时拆成 `plan`、`apply`、`snapshot`、`ownership`、`merge`（按键合并）几个子模块。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::SnapshotId;
use crate::{Error, Result};

/// 变更计划里一行的操作，对应 6.8 线框图的 `+`、`~`、`−`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum ChangeOp {
    Add,
    Modify,
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Change {
    pub op: ChangeOp,
    pub path: PathBuf,
    /// 这一行做什么，例如「链接到 tdd@a1b2c3」「新增键 mcpServers.github」。
    pub detail: String,
}

/// 变更计划（6.8）。同步、收编、升级、回滚、转成套装、从全局移除都用它。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChangePlan {
    /// 标题，例如「即将对 evals_studio 做 4 处变更」。
    pub title: String,
    pub changes: Vec<Change>,
}

/// 应用一份计划的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Applied {
    /// 受影响的每个范围各一个快照。
    pub snapshots: Vec<SnapshotId>,
}

/// 按清单与锁算出要对项目做的变更。只算不写。
pub fn plan_sync(_project_root: &Path) -> Result<ChangePlan> {
    Err(Error::NotImplemented("materializer::plan_sync"))
}

/// 算出回滚到某个快照要做的变更。后来的变更动过同一批文件时，计划里标出来，
/// 并要求先回滚后面的记录，不做自动合并（9.3）。
pub fn plan_rollback(_snapshot: SnapshotId) -> Result<ChangePlan> {
    Err(Error::NotImplemented("materializer::plan_rollback"))
}

/// 应用计划：校验锁里的内容哈希、留快照、写入、记活动。
///
/// 应用前重新核对磁盘现状，和出计划时不一样就拒绝，让调用方重新出计划。
pub fn apply(_plan: &ChangePlan) -> Result<Applied> {
    Err(Error::NotImplemented("materializer::apply"))
}
