use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 四层作用域（4.3）。声明顺序就是覆盖顺序：后面的覆盖前面的。
///
/// 界面的「层级刻度」四个小方格按这个顺序排。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum Scope {
    /// 全局基线。
    Global,
    /// 套装。
    Bundle,
    /// 项目清单 `fangcun.toml`。
    Project,
    /// 个人覆盖 `fangcun.local.toml`。
    Personal,
}

impl Scope {
    pub const ALL: [Scope; 4] = [
        Scope::Global,
        Scope::Bundle,
        Scope::Project,
        Scope::Personal,
    ];

    /// 下层覆盖上层。
    pub fn overrides(self, other: Scope) -> bool {
        self > other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_layers_override_upper_layers() {
        assert!(Scope::Personal.overrides(Scope::Project));
        assert!(Scope::Project.overrides(Scope::Bundle));
        assert!(Scope::Bundle.overrides(Scope::Global));
        assert!(!Scope::Global.overrides(Scope::Project));
    }
}
