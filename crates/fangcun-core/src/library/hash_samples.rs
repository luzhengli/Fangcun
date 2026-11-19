//! 内容哈希的边界样例。`content.rs` 的测试和 CI 里与 `skills` 命令行逐字节比对的那一步
//! （W-13，`examples/hash_crosscheck.rs`）用的是同一组文件，改了两边一起变。
//!
//! 只给测试和比对用，不是内核的功能。

use std::fs;
use std::io;
use std::path::Path;

/// v2 的边界样例写进 `skill`：CRLF 换 LF、含 NUL 的不换、三种跳过的目录与任意层级的
/// `metadata.json`、不跳过的 `.DS_Store` 和 `node_modules`、`a-b` `a.b` `a/b` 的字节序、
/// 非 ASCII 文件名、目录内的文件与目录软链、指向目录外（`skill` 的上一级的 `outside.txt`）、
/// 断掉和指向自身的软链、空目录。
#[cfg(unix)]
pub fn write_v2_sample(skill: &Path) -> io::Result<()> {
    use std::os::unix::fs::symlink;

    let write = |rel: &str, bytes: &[u8]| -> io::Result<()> {
        let path = skill.join(rel);
        fs::create_dir_all(path.parent().unwrap_or(skill))?;
        fs::write(path, bytes)
    };
    write("SKILL.md", b"---\r\nname: demo\r\n---\r\nbody\r\n")?;
    write("a-b.md", b"dash\n")?;
    write("a.b.md", b"dot\n")?;
    write("a/b.md", b"slash\n")?;
    write("中文.md", "unicode\n".as_bytes())?;
    write("bin.dat", b"\0\r\n\x01")?;
    write("metadata.json", b"{}")?;
    write("sub/metadata.json", b"{}")?;
    write("sub/note.md", b"nested\r\n")?;
    write("sub/__pycache__/x.pyc", b"pyc")?;
    write(".git/HEAD", b"ref")?;
    write("__pypackages__/y", b"y")?;
    write(".DS_Store", b"ds")?;
    write("node_modules/m.js", b"m")?;
    write("scripts/run.sh", b"echo hi\n")?;
    fs::create_dir_all(skill.join("empty"))?;
    if let Some(parent) = skill.parent() {
        fs::write(parent.join("outside.txt"), b"out")?;
    }
    symlink("SKILL.md", skill.join("inner-link.md"))?;
    symlink("sub", skill.join("linkdir"))?;
    symlink("../outside.txt", skill.join("outside.md"))?;
    symlink(".", skill.join("self"))?;
    symlink("missing", skill.join("dangling"))
}

/// v1（旧算法）的边界样例写进 `dir`：大写排在同字母的小写后面、`_` 在 `-` 前、`.` 在字母前、
/// `metadata.json` 照算、`.git` 与 `node_modules` 跳过、软链不算、CRLF 不换。
#[cfg(unix)]
pub fn write_v1_sample(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::symlink;

    let write = |rel: &str, bytes: &[u8]| -> io::Result<()> {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap_or(dir))?;
        fs::write(path, bytes)
    };
    write("SKILL.md", b"---\nname: demo\n---\n")?;
    write("scripts/main.ts", b"x\r\n")?;
    write("README.md", b"r")?;
    write("a-b.md", b"1")?;
    write("a_b.md", b"2")?;
    write("Z.md", b"3")?;
    write("metadata.json", b"{}")?;
    write(".git/HEAD", b"ref")?;
    write("node_modules/m.js", b"m")?;
    symlink("SKILL.md", dir.join("link.md"))
}
