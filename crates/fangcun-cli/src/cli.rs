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
    /// 只读模式：拒绝一切写入，和设置里的「只读模式」一样（7.5）
    #[arg(long, global = true)]
    pub read_only: bool,

    /// 不问，直接应用变更计划（第 5 节）
    #[arg(long, global = true)]
    pub yes: bool,

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

    /// 从全局层移除（只对已收编进资产库的项，6.6、7.5）
    Remove {
        /// 资产，可以写几个，名字可以带 * 通配
        #[arg(required = true)]
        assets: Vec<String>,

        /// 从全局层移除。目前只支持这一种
        #[arg(long, required = true)]
        global: bool,

        /// 只动这个工具的全局层，例如 claude-code。不写时是全部完整支持的工具
        #[arg(long)]
        tool: Option<String>,
    },

    /// 重新解析清单，更新锁
    Lock,

    /// 导出成别的工具的格式
    Export {
        #[command(subcommand)]
        format: ExportFormat,
    },

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

    /// 管理钥匙串里的密钥（F6）。值只在终端里输入，不回显、不记录
    Secret {
        #[command(subcommand)]
        action: SecretAction,
    },
}

/// `fangcun secret` 的三个动作。
#[derive(Debug, Subcommand)]
pub enum SecretAction {
    /// 存入或替换。终端里输入，不回显；不在终端里时从标准输入读一行
    Set {
        /// 名称，即清单里 secret:<名称> 的那个
        name: String,
    },
    /// 删除
    Rm {
        /// 名称
        name: String,
    },
    /// 列出已登记项目引用的密钥和钥匙串里存着的，不显示值
    Ls,
}

/// `fangcun export` 能导出的格式。
#[derive(Debug, Subcommand)]
pub enum ExportFormat {
    /// 把锁导出成 skills-lock.json，与 skills 命令行互通
    SkillsLock,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn definition_is_consistent() {
        Cli::command().debug_assert();
    }

    /// 设计文档第 5 节列出的十三个命令，顺序一致；另加 `remove`（批 6，Agent 定，待写进第 5 节）。
    #[test]
    fn subcommands_match_the_design_doc() {
        let names: Vec<String> = Cli::command()
            .get_subcommands()
            .map(|c| c.get_name().to_owned())
            .collect();
        assert_eq!(
            names,
            [
                "doctor", "init", "status", "add", "adopt", "remove", "lock", "export", "sync",
                "update", "rollback", "log", "why", "secret"
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
            "fangcun remove lark-* --global",
            "fangcun remove frontend-design context7 --global --tool claude-code",
            "fangcun lock",
            "fangcun export skills-lock",
            "fangcun sync",
            "fangcun sync --frozen",
            "fangcun update",
            "fangcun update tdd --dry-run --projects evals_studio,client-acme",
            "fangcun rollback 126",
            "fangcun log",
            "fangcun why tdd",
            "fangcun why lark-doc --global",
            "fangcun secret set github-token",
            "fangcun secret rm github-token",
            "fangcun secret ls",
        ] {
            Cli::try_parse_from(line.split_whitespace())
                .unwrap_or_else(|e| panic!("`{line}` 应能解析：{e}"));
        }
    }
}
