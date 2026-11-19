use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// 变更计划上方的一条提示（6.8）：计划没做、做不到、要人留意的事。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanNote {
    pub tone: NoteTone,
    pub text: String,
}

/// 提示的轻重，对应「墨格」的语义色：说明用灰，要留意用橙，有风险用红。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export)]
pub enum NoteTone {
    Info,
    Warning,
    Danger,
}

impl PlanNote {
    pub fn info(text: impl Into<String>) -> Self {
        Self {
            tone: NoteTone::Info,
            text: text.into(),
        }
    }

    pub fn warning(text: impl Into<String>) -> Self {
        Self {
            tone: NoteTone::Warning,
            text: text.into(),
        }
    }

    pub fn danger(text: impl Into<String>) -> Self {
        Self {
            tone: NoteTone::Danger,
            text: text.into(),
        }
    }
}
