//! 路径沙箱与敏感路径检测（对齐 `biz.agent.safety`）。

use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static SENSITIVE: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
        r"(^|/)(\.env|\.env\.[^/]+)$",
        r"(^|/)id_rsa(\.pub)?$",
        r"(^|/)\.git/",
        r"(^|/)\.ssh/",
        r"\.pem$",
        r"\.key$",
    ]
    .into_iter()
    .map(|p| Regex::new(p).expect("sensitive regex"))
    .collect()
});

pub fn is_path_safe(candidate: impl AsRef<Path>, repo_root: impl AsRef<Path>) -> bool {
    let Ok(cand) = normalize(candidate.as_ref()) else {
        return false;
    };
    let Ok(root) = normalize(repo_root.as_ref()) else {
        return false;
    };
    cand.starts_with(&root)
}

pub fn is_sensitive_path(path: impl AsRef<Path>) -> bool {
    let posix = path.as_ref().to_string_lossy().replace('\\', "/");
    SENSITIVE.iter().any(|re| re.is_match(&posix))
}

fn normalize(path: &Path) -> Result<PathBuf, std::io::Error> {
    use std::path::{Component, PathBuf as Pb};
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut out = Pb::new();
    for c in abs.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_escape() {
        let root = std::env::temp_dir().join("xiezhi-safe-root");
        let _ = std::fs::create_dir_all(&root);
        assert!(is_path_safe(root.join("a.rs"), &root));
        assert!(!is_path_safe(root.join("..").join("outside"), &root));
    }

    #[test]
    fn detects_env() {
        assert!(is_sensitive_path(".env"));
        assert!(is_sensitive_path("conf/.env.local"));
        assert!(!is_sensitive_path("src/main.rs"));
    }
}
