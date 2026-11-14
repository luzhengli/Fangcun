//! 领域模型：纯数据，不做 IO。概念定义见设计文档 4.2–4.4。
//!
//! 带 `#[ts(export)]` 的类型会导出成 TypeScript，供桌面界面使用。
//! 改了这里的类型后运行 `pnpm bindings`。

mod asset;
mod scope;
mod status;
mod version;

pub use asset::{AssetKind, ObservedKind, ProjectId, SnapshotId, SourceRef, ToolId};
pub use scope::Scope;
pub use status::{AssetState, Timestamp, TokenEstimate};
pub use version::{ContentHash, SyncMode, Version, VersionPolicy};
