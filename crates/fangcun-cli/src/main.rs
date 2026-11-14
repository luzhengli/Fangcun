//! 命令行 `fangcun`。
//!
//! 这一层只做三件事：解析参数、调内核的一个用例、把结果打印出来。
//! 业务逻辑一律放在 `fangcun-core`，这样界面能做的事命令行都能做，行为也一致。

mod cli;
mod commands;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();
    match commands::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fangcun: {error}");
            ExitCode::from(exit_code(&error))
        }
    }
}

/// 退出码。CI 靠它判断结果，定了就不要改。
///
/// - 1：运行失败，包括 `sync --frozen` 校验不通过。
/// - 2：参数错误，clap 自己返回。
/// - 3：这个命令还没实现。
fn exit_code(error: &fangcun_core::Error) -> u8 {
    match error {
        fangcun_core::Error::NotImplemented(_) => 3,
        _ => 1,
    }
}
