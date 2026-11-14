use std::path::Path;

use crate::{Error, Result};

/// `fangcun.lock`：每项资产解析出的确切提交与内容哈希（4.2）。
///
/// 文件格式文档还没有写。实现解析器写锁（M1）前，先把格式写进设计文档 7.4，
/// 再在这里补字段。M0 不读也不写锁。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lock {}

impl Lock {
    pub fn load(_project_root: &Path) -> Result<Option<Self>> {
        Err(Error::NotImplemented("manifest::Lock::load"))
    }
}
