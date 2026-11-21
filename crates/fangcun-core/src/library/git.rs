//! 调本机的 `git`。用本机的 git，私有仓库走用户已有的凭据，不另存密码。
//!
//! 一律关掉交互式提示（`GIT_TERMINAL_PROMPT=0`），要密码时直接失败，不会卡住界面。

use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{Error, Result};

fn command(dir: Option<&Path>) -> Command {
    let mut cmd = Command::new("git");
    if let Some(dir) = dir {
        cmd.arg("-C").arg(dir);
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    cmd
}

/// 跑一条 git 命令，返回标准输出。失败时把 git 的报错带出来。
pub(super) fn run(dir: Option<&Path>, args: &[&str]) -> Result<Vec<u8>> {
    let output = command(dir)
        .args(args)
        .output()
        .map_err(|e| Error::Source(format!("运行 git 失败：{e}。请确认本机装了 git")))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(Error::Source(format!(
            "git {} 失败：{}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

/// 带环境变量跑一条 git 命令（回流时用临时索引、固定的提交时间）。
pub(super) fn run_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<String> {
    let mut cmd = command(Some(dir));
    for (k, v) in env {
        cmd.env(k, v);
    }
    let output = cmd
        .args(args)
        .output()
        .map_err(|e| Error::Source(format!("运行 git 失败：{e}。请确认本机装了 git")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(Error::Source(format!(
            "git {} 失败：{}",
            args.first().copied().unwrap_or_default(),
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

pub(super) fn run_text(dir: Option<&Path>, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8_lossy(&run(dir, args)?).trim().to_owned())
}

/// 解析成提交。解析不到时是 `None`，不算错。
pub(super) fn commit_of(repo: &Path, rev: &str) -> Option<String> {
    let spec = format!("{rev}^{{commit}}");
    run_text(
        Some(repo),
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &spec,
        ],
    )
    .ok()
    .filter(|s| !s.is_empty())
}

/// 把仓库里某个提交的一个目录导出到 `to`（`to` 已存在且为空）。`dir` 为空表示仓库根目录。
pub(super) fn export(repo: &Path, commit: &str, dir: &str, to: &Path) -> Result<()> {
    let tree = if dir.is_empty() {
        commit.to_owned()
    } else {
        format!("{commit}:{dir}")
    };
    let mut archive = command(Some(repo))
        .args(["archive", "--format=tar", "--end-of-options", &tree])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Source(format!("运行 git 失败：{e}")))?;
    let stdout = archive
        .stdout
        .take()
        .ok_or_else(|| Error::Source("git archive 没有输出".to_owned()))?;
    let tar = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(to)
        .stdin(stdout)
        .output()
        .map_err(|e| Error::Source(format!("运行 tar 失败：{e}")))?;
    let archived = archive
        .wait_with_output()
        .map_err(|e| Error::Source(format!("运行 git 失败：{e}")))?;
    if !archived.status.success() {
        return Err(Error::Source(format!(
            "git archive 失败：{}",
            String::from_utf8_lossy(&archived.stderr).trim()
        )));
    }
    if !tar.status.success() {
        return Err(Error::Source(format!(
            "解包失败：{}",
            String::from_utf8_lossy(&tar.stderr).trim()
        )));
    }
    Ok(())
}

/// 一次读出多个对象的内容（`git cat-file --batch`），比逐个 `git show` 快得多。
/// `specs` 是 `<提交>:<路径>`；不存在的对象是 `None`。
pub(super) fn cat_batch(repo: &Path, specs: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
    if specs.is_empty() {
        return Ok(Vec::new());
    }
    let mut child = command(Some(repo))
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| Error::Source(format!("运行 git 失败：{e}")))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| Error::Source("git cat-file 没有输入".to_owned()))?;
    let input: String = specs.iter().map(|s| format!("{s}\n")).collect();
    // 另起一个线程写，免得输出塞满管道时两边互等。
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Source("git cat-file 没有输出".to_owned()))?;
    let mut reader = BufReader::new(stdout);
    let mut out = Vec::with_capacity(specs.len());
    let broken = || Error::Source("git cat-file 的输出不完整".to_owned());
    for _ in specs {
        let mut header = String::new();
        if reader.read_line(&mut header).map_err(|_| broken())? == 0 {
            return Err(broken());
        }
        let header = header.trim_end();
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            out.push(None);
            continue;
        }
        let size: usize = header
            .rsplit(' ')
            .next()
            .and_then(|n| n.parse().ok())
            .ok_or_else(broken)?;
        let mut content = vec![0; size + 1];
        reader.read_exact(&mut content).map_err(|_| broken())?;
        content.pop();
        out.push(Some(content));
    }
    let _ = writer.join();
    let _ = child.wait();
    Ok(out)
}
