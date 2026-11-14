//! 命令行的参数定义。命令和选项照设计文档第 5 节「配套：命令行」，不多不少。
//! 要加命令或选项，先改设计文档。

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "fangcun",
    version,
    about = "方寸：Agent 上下文的包管理器与环境管理器"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// 只读体检
    Doctor,

    /// 登记项目，生成清单与锁
    Init {
        /// 从目录现状反推清单与锁
        #[arg(long)]
        from_existing: bool,
    },

    /// 当前项目的同步状态与漂移
    Status,

    /// 加入清单
    Add {
        /// 资产
        asset: String,
    },

    /// 收编进资产库
    Adopt {
        /// 资产
        asset: String,
    },

    /// 重新解析清单，更新锁
    Lock,

    /// 按清单与锁物化
    Sync {
        /// CI 用：锁与清单不一致就失败
        #[arg(long)]
        frozen: bool,
    },

    /// 检查并升级
    Update {
        /// 资产。不写表示全部
        asset: Option<String>,

        /// 只看差异
        #[arg(long)]
        dry_run: bool,

        /// 指定项目
        #[arg(long, value_delimiter = ',')]
        projects: Vec<String>,
    },

    /// 回滚到某个快照
    Rollback {
        /// 快照编号
        snapshot: u32,
    },

    /// 查看活动记录
    Log,

    /// 解释一项资产为何生效、来自哪一层
    Why {
        /// 资产
        asset: String,

        /// 看全局层
        #[arg(long)]
        global: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn definition_is_consistent() {
        Cli::command().debug_assert();
    }

    /// 设计文档第 5 节列出的十一个命令，顺序一致。
    #[test]
    fn subcommands_match_the_design_doc() {
        let names: Vec<String> = Cli::command()
            .get_subcommands()
            .map(|c| c.get_name().to_owned())
            .collect();
        assert_eq!(
            names,
            [
                "doctor", "init", "status", "add", "adopt", "lock", "sync", "update", "rollback",
                "log", "why"
            ]
        );
    }

    #[test]
    fn documented_invocations_parse() {
        for line in [
            "fangcun doctor",
            "fangcun init --from-existing",
            "fangcun status",
            "fangcun add tdd",
            "fangcun adopt project-harness",
            "fangcun lock",
            "fangcun sync",
            "fangcun sync --frozen",
            "fangcun update",
            "fangcun update tdd --dry-run --projects evals_studio,client-acme",
            "fangcun rollback 126",
            "fangcun log",
            "fangcun why tdd",
            "fangcun why lark-doc --global",
        ] {
            Cli::try_parse_from(line.split_whitespace())
                .unwrap_or_else(|e| panic!("`{line}` 应能解析：{e}"));
        }
    }
}
