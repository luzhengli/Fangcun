//! 命令行 `fangcun`。
//!
//! 这一层只做三件事：解析参数、调内核的一个用例、把结果打印出来。
//! 业务逻辑一律放在 `fangcun-core`，这样界面能做的事命令行都能做，行为也一致。

mod cli;
mod commands;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    quiet_on_broken_pipe();
    let cli = cli::Cli::parse();
    match commands::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fangcun: {error}");
            ExitCode::from(exit_code(&error))
        }
    }
}

/// `fangcun log | head` 这类用法：读的一端先关了，`println!` 会 panic 并打印一段吓人的报错。
/// 照没有 Rust 时的惯例安静退出，退出码 141（128 + SIGPIPE）。工作区禁用 unsafe，
/// 不能直接把 SIGPIPE 恢复成默认，所以在 panic hook 里认出这一种。
fn quiet_on_broken_pipe() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .unwrap_or_default();
        if message.contains("failed printing to stdout") && message.contains("Broken pipe") {
            std::process::exit(141);
        }
        default(info);
    }));
}

/// 退出码。CI 靠它判断结果，定了就不要改。
///
/// - 1：运行失败，包括 `sync --frozen` 校验不通过。
/// - 2：参数错误，clap 自己返回。
/// - 3：这个命令还没实现。
/// - 141：输出被下游提前关掉（例如接了 `head`），见 [`quiet_on_broken_pipe`]。
fn exit_code(error: &fangcun_core::Error) -> u8 {
    match error {
        fangcun_core::Error::NotImplemented(_) => 3,
        _ => 1,
    }
}
