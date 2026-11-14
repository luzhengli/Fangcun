//! 估算器（7.2）：本地估算 token。
//!
//! 结果永远是估算值，重在相对比较而非绝对数值（9.1）。不联网，不调用任何模型接口。
//!
//! 口径（M0-06，见 DECISIONS 2026-11-14「M0 实现」）：
//! - 一段文本：ASCII 字符每 4 个算 1 个 token，其余字符（中文等）每个算 1 个，向上取整。
//! - 「常驻」算每次会话都会进上下文的部分：
//!   - 指令文件：全文。
//!   - Skill：名称加描述，即 [`skill_listing`] 拼出的那一行。
//!   - MCP：工具定义要启动服务才读得到，M0 不启动任何进程，所以不估算。

use crate::model::TokenEstimate;

/// 估算一段文本的 token 数。
pub fn estimate_text(text: &str) -> TokenEstimate {
    let mut ascii = 0u64;
    let mut other = 0u64;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii += 1;
        } else {
            other += 1;
        }
    }
    let tokens = ascii.div_ceil(4) + other;
    TokenEstimate(u32::try_from(tokens).unwrap_or(u32::MAX))
}

/// Skill 常驻在上下文里的那一行：名称与描述。
pub fn skill_listing(name: &str, description: Option<&str>) -> String {
    match description {
        Some(description) => format!("- {name}: {description}"),
        None => format!("- {name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_counts_four_characters_per_token() {
        assert_eq!(estimate_text("abcd").0, 1);
        assert_eq!(estimate_text("abcde").0, 2);
        assert_eq!(estimate_text("").0, 0);
    }

    #[test]
    fn other_characters_count_one_each() {
        assert_eq!(estimate_text("方寸").0, 2);
        assert_eq!(estimate_text("方寸 ok").0, 3);
    }
}
