//! CI 用（W-13）：造一个「用 `skills` 命令行装过 skill 的项目」，让真正的 `skills` 命令行读它的
//! 锁，比对内容哈希和方寸是否逐字节一致。
//!
//! 在 `<目录>` 下写：
//! - `.agents/skills/<样例>`：`library::hash_samples` 的边界样例，和 `content.rs` 的测试同一组；
//! - `skills-lock.json`：版本 1，每项的哈希是方寸按旧算法（v1）算的；
//! - `expected.json`：每项按新算法（v2）算的哈希。
//!
//! `skills` 读版本 1 的锁时先按旧算法校验已安装的目录，对得上才换算成新算法的哈希，对不上就置空。
//! 所以它换算出来的哈希等于 `expected.json`，就说明新旧两种算法都和它一致。
//! 比对在 `tools/hash-crosscheck/check.mjs`。
//!
//! 用法：cargo run -p fangcun-core --example hash_crosscheck -- <目录>

#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use fangcun_core::library::content::{hash_dir, legacy_hash_dir};
    use fangcun_core::library::hash_samples::{write_v1_sample, write_v2_sample};
    use serde_json::{Map, Value, json};

    let root = std::path::PathBuf::from(std::env::args().nth(1).ok_or("要给出目录")?);
    let _ = std::fs::remove_dir_all(&root);
    let installed = root.join(".agents/skills");
    std::fs::create_dir_all(&installed)?;
    write_v2_sample(&installed.join("boundary-v2"))?;
    write_v1_sample(&installed.join("boundary-v1"))?;

    let mut lock = Map::new();
    let mut expected = Map::new();
    for name in ["boundary-v1", "boundary-v2"] {
        let dir = installed.join(name);
        lock.insert(
            name.to_owned(),
            json!({
                "source": "acme/skills",
                "sourceType": "github",
                "computedHash": legacy_hash_dir(&dir).0,
            }),
        );
        expected.insert(name.to_owned(), Value::String(hash_dir(&dir).0));
    }
    let lock = json!({ "version": 1, "skills": lock });
    std::fs::write(
        root.join("skills-lock.json"),
        serde_json::to_string_pretty(&lock)?,
    )?;
    std::fs::write(
        root.join("expected.json"),
        serde_json::to_string_pretty(&Value::Object(expected))?,
    )?;
    println!("{}", root.display());
    Ok(())
}

#[cfg(not(unix))]
fn main() {}
