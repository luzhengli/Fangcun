//! 方寸内核。
//!
//! 桌面应用（Tauri）与命令行 `fangcun` 都只通过 [`Kernel`] 调用这里，
//! 所以两边的行为一致。模块划分对应设计文档 7.2，工程约定见
//! `docs/ARCHITECTURE.md`。
//!
//! 依赖方向（上面的可以用下面的，反过来不行）：
//!
//! ```text
//! kernel                                     对外门面，一个用例一个方法
//! scanner  resolver  global  materializer  watcher   用例层
//! library  manifest  journal  estimator  secrets
//! adapter                                    各工具的声明式描述
//! model  paths  error                        纯类型，不做 IO
//! ```

pub mod adapter;
pub mod error;
pub mod estimator;
pub mod global;
pub mod journal;
pub mod kernel;
pub mod library;
pub mod manifest;
pub mod materializer;
pub mod model;
pub mod paths;
pub mod resolver;
pub mod scanner;
pub mod secrets;
pub mod watcher;

pub use error::{Error, Result};
pub use kernel::{Access, Kernel};
