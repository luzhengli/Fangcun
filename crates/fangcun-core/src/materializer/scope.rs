//! 写入范围（7.5）：方寸自己的目录、已登记的项目、适配器声明的工具全局位置。
//!
//! 范围外的路径一律拒绝。判断前先把路径落到真实位置：父目录跟随软链，最后一段不跟随，
//! 因为替换软链本身是常见的写法（例如把 `~/.claude/skills/tdd` 换成指向资产库的链接）。

use std::path::{Component, Path, PathBuf};

use crate::adapter::{AdapterRegistry, SupportLevel, expand};
use crate::{Error, Result};

/// 一个路径落在哪个范围。快照按范围分组（6.7「快照」）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Target {
    /// 方寸自己的 `~/.fangcun`。
    Own,
    /// 某个已登记项目，值是项目根目录。
    Project(PathBuf),
    /// 工具的全局层（含个人层里不在项目内的文件，例如 `~/.claude.json`）。
    Global,
}

#[derive(Debug, Clone)]
pub struct WriteScope {
    own: PathBuf,
    home: PathBuf,
    projects: Vec<PathBuf>,
    global: Vec<PathBuf>,
}

impl WriteScope {
    /// - `own`：`~/.fangcun`。
    /// - 项目：已登记的项目根目录。主目录本身、主目录的上层、`~/.fangcun` 里面的不算，
    ///   免得一条登记就把整个主目录放进范围。
    /// - 全局：完整支持的工具（Claude Code、Codex）在适配器里声明的全局层与个人层位置，
    ///   只认主目录下的。企业托管（`/Library/Application Support/ClaudeCode`）和
    ///   `/etc/codex` 不归用户，不写。
    pub fn new(
        own: &Path,
        user_home: &Path,
        adapters: &AdapterRegistry,
        projects: impl IntoIterator<Item = PathBuf>,
    ) -> Self {
        let own = resolve(own);
        let home = resolve(user_home);
        let global = adapters
            .iter()
            .filter(|adapter| adapter.tool.support == SupportLevel::Full)
            .flat_map(global_specs)
            .filter_map(|spec| expand(&spec, user_home, None))
            .map(|path| resolve(&path))
            .filter(|path| path.starts_with(&home) && *path != home)
            .collect();
        let mut scope = Self {
            own,
            home,
            projects: Vec::new(),
            global,
        };
        scope.add_projects(projects);
        scope
    }

    /// 加进项目。计划里登记的项目在应用时算进来。
    pub fn add_projects(&mut self, roots: impl IntoIterator<Item = PathBuf>) {
        for root in roots {
            let root = resolve(&root);
            if !self.home.starts_with(&root) && !root.starts_with(&self.own) {
                self.projects.push(root);
            }
        }
    }

    /// 路径在不在范围里，在哪个范围。
    ///
    /// 拒绝的情况：相对路径、带 `.` 或 `..` 的路径、项目里 `.git` 下的路径、不在任何范围里的路径。
    pub fn check(&self, path: &Path) -> Result<Target> {
        let refuse = || Error::OutOfScope(path.to_path_buf());
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
        {
            return Err(refuse());
        }
        let real = resolve(path);
        if real.starts_with(&self.own) {
            return Ok(Target::Own);
        }
        // 嵌套的项目取最深的那个。
        if let Some(root) = self
            .projects
            .iter()
            .filter(|root| real.starts_with(root))
            .max_by_key(|root| root.components().count())
        {
            let inside_git = real
                .strip_prefix(root)
                .is_ok_and(|rest| rest.components().any(|c| c.as_os_str() == ".git"));
            if inside_git {
                return Err(refuse());
            }
            return Ok(Target::Project(root.clone()));
        }
        if self.global.iter().any(|root| real.starts_with(root)) {
            return Ok(Target::Global);
        }
        Err(refuse())
    }
}

impl WriteScope {
    /// 建一个还不存在的目录。除了 [`check`](Self::check) 认的路径，还允许建全局位置的上层目录，
    /// 例如没装过 Codex 时先建 `~/.codex`，再写 `~/.codex/config.toml`。不超出主目录。
    pub fn check_create_dir(&self, path: &Path) -> Result<Target> {
        let refused = match self.check(path) {
            Ok(target) => return Ok(target),
            Err(error) => error,
        };
        let real = resolve(path);
        let above_global = real.starts_with(&self.home)
            && real != self.home
            && self.global.iter().any(|root| root.starts_with(&real));
        if above_global
            && !path
                .components()
                .any(|c| matches!(c, Component::CurDir | Component::ParentDir))
        {
            Ok(Target::Global)
        } else {
            Err(refused)
        }
    }
}

/// 适配器里全局层与个人层的位置写法。项目内的相对写法在 `expand` 时得到 `None`，自然排除。
fn global_specs(adapter: &crate::adapter::Adapter) -> Vec<String> {
    use crate::model::Scope;
    let wanted = |scope: Scope| matches!(scope, Scope::Global | Scope::Personal);
    let mut specs = Vec::new();
    specs.extend(
        adapter
            .skills
            .iter()
            .filter(|l| wanted(l.scope))
            .map(|l| l.path.clone()),
    );
    for location in adapter.instructions.iter().filter(|l| wanted(l.scope)) {
        specs.extend(location.names.iter().cloned());
        specs.extend(location.personal.iter().cloned());
    }
    specs.extend(
        adapter
            .rules
            .iter()
            .filter(|l| wanted(l.scope))
            .map(|l| l.dir.clone()),
    );
    specs.extend(
        adapter
            .mcp
            .iter()
            .filter(|l| wanted(l.scope))
            .map(|l| l.file.clone()),
    );
    specs.extend(
        adapter
            .skill_switches
            .iter()
            .filter(|l| wanted(l.scope))
            .map(|l| l.file.clone()),
    );
    specs
}

/// 落到真实位置：最深的已存在的上层目录跟随软链，后面的部分原样接上；最后一段不跟随。
pub(super) fn resolve(path: &Path) -> PathBuf {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return path.to_path_buf();
    };
    let mut existing = parent;
    let mut rest = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(existing) {
            let mut out = real;
            out.extend(rest.iter().rev());
            out.push(name);
            return out;
        }
        match (existing.parent(), existing.file_name()) {
            (Some(up), Some(last)) => {
                rest.push(last.to_os_string());
                existing = up;
            }
            _ => return path.to_path_buf(),
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    struct Fixture {
        base: PathBuf,
        scope: WriteScope,
    }

    fn fixture(name: &str) -> Fixture {
        let base =
            std::env::temp_dir().join(format!("fangcun-scope-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let home = base.join("home");
        for dir in [
            "home/.fangcun",
            "home/.claude/skills",
            "home/proj/.git",
            "home/other",
        ] {
            fs::create_dir_all(base.join(dir)).unwrap();
        }
        let adapters = AdapterRegistry::builtin().unwrap();
        let scope = WriteScope::new(
            &home.join(".fangcun"),
            &home,
            &adapters,
            [home.join("proj"), home.clone(), PathBuf::from("/")],
        );
        Fixture { base, scope }
    }

    #[test]
    fn classifies_own_project_and_global_paths() {
        let f = fixture("classify");
        let home = f.base.join("home");
        assert_eq!(
            f.scope.check(&home.join(".fangcun/projects.toml")).unwrap(),
            Target::Own
        );
        assert!(matches!(
            f.scope
                .check(&home.join("proj/.claude/skills/tdd"))
                .unwrap(),
            Target::Project(_)
        ));
        assert_eq!(
            f.scope
                .check(&home.join(".claude/skills/tdd/SKILL.md"))
                .unwrap(),
            Target::Global
        );
        // Codex 的全局配置还不存在，也在范围里。
        assert_eq!(
            f.scope.check(&home.join(".codex/config.toml")).unwrap(),
            Target::Global
        );
        let _ = fs::remove_dir_all(&f.base);
    }

    #[test]
    fn refuses_everything_else() {
        let f = fixture("refuse");
        let home = f.base.join("home");
        for path in [
            home.join("other/file"),
            home.join(".zshrc"),
            home.join("proj/.git/config"),
            home.join("proj/../other/file"),
            PathBuf::from("relative/path"),
            PathBuf::from("/Library/Application Support/ClaudeCode/CLAUDE.md"),
            PathBuf::from("/etc/codex/skills/x"),
        ] {
            assert!(
                matches!(f.scope.check(&path), Err(Error::OutOfScope(_))),
                "{} 不该在范围里",
                path.display()
            );
        }
        let _ = fs::remove_dir_all(&f.base);
    }

    #[test]
    fn may_create_the_missing_parents_of_a_global_location() {
        let f = fixture("parents");
        let home = f.base.join("home");
        assert_eq!(
            f.scope.check_create_dir(&home.join(".codex")).unwrap(),
            Target::Global
        );
        assert!(
            f.scope.check(&home.join(".codex")).is_err(),
            "只许建目录，不许写文件"
        );
        assert!(f.scope.check_create_dir(&home).is_err());
        assert!(f.scope.check_create_dir(&home.join("other/dir")).is_err());
        let _ = fs::remove_dir_all(&f.base);
    }

    #[test]
    fn symlinked_parents_cannot_escape_the_scope() {
        let f = fixture("escape");
        let home = f.base.join("home");
        symlink(home.join("other"), home.join("proj/sneaky")).unwrap();
        assert!(f.scope.check(&home.join("proj/sneaky/file")).is_err());
        // 最后一段是软链时不跟随：替换的是链接本身，仍在项目里。
        symlink(home.join("other"), home.join("proj/link")).unwrap();
        assert!(matches!(
            f.scope.check(&home.join("proj/link")).unwrap(),
            Target::Project(_)
        ));
        let _ = fs::remove_dir_all(&f.base);
    }
}
