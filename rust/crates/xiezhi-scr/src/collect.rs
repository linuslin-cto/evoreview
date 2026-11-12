//! 本地 Git Diff 采集（对齐 Python `biz.scr.git_diff` 采集入口）。

use crate::git_diff::{decide_path, split_unified_diffs, to_changes, LocalDiffBundle};
use crate::rules::RuleResolver;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CollectError {
    #[error("git 命令失败: {0}")]
    Git(String),
    #[error("{0}")]
    InvalidArgs(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

fn run_git(repo_dir: &Path, args: &[&str], check: bool) -> Result<String, CollectError> {
    let output = Command::new("git")
        .args(["-c", "core.quotepath=false"])
        .args(args)
        .current_dir(repo_dir)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    if check && !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let msg = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return Err(CollectError::Git(format!(
            "git {} 失败: {msg}",
            args.join(" ")
        )));
    }
    Ok(stdout)
}

fn untracked_as_diff(repo_dir: &Path, path: &str) -> String {
    let abs = repo_dir.join(path);
    let Ok(content) = fs::read_to_string(&abs) else {
        return String::new();
    };
    if content.contains('\0') {
        return String::new();
    }
    let lines: Vec<&str> = content.lines().collect();
    let mut out = vec![
        "--- /dev/null".to_string(),
        format!("+++ b/{path}"),
        format!("@@ -0,0 +1,{} @@", lines.len()),
    ];
    for line in &lines {
        out.push(format!("+{line}"));
    }
    if !lines.is_empty() {
        out.push(String::new());
    }
    out.join("\n")
}

fn build_preview(
    diff_map: &BTreeMap<String, String>,
    rules: &RuleResolver,
) -> Vec<crate::git_diff::DiffPreviewItem> {
    diff_map
        .iter()
        .map(|(path, diff)| decide_path(path, diff, rules))
        .collect()
}

pub fn collect_workspace_diffs(
    repo_dir: &Path,
    rules: &RuleResolver,
) -> Result<LocalDiffBundle, CollectError> {
    let head_raw = run_git(
        repo_dir,
        &["diff", "HEAD", "--no-ext-diff", "--text", "--unified=3"],
        false,
    )?;
    let untracked_raw = run_git(
        repo_dir,
        &["ls-files", "--others", "--exclude-standard"],
        false,
    )?;

    let mut diff_map: BTreeMap<String, String> =
        split_unified_diffs(&head_raw).into_iter().collect();
    for line in untracked_raw.lines() {
        let path = line.trim();
        if path.is_empty() || diff_map.contains_key(path) {
            continue;
        }
        diff_map.insert(path.to_string(), untracked_as_diff(repo_dir, path));
    }

    let preview = build_preview(&diff_map, rules);
    let changes = to_changes(&preview, &diff_map);
    Ok(LocalDiffBundle {
        mode: "workspace".into(),
        repo_dir: repo_dir.display().to_string(),
        changes,
        preview,
        warnings: Vec::new(),
    })
}

pub fn collect_commit_diffs(
    repo_dir: &Path,
    commit: &str,
    rules: &RuleResolver,
) -> Result<LocalDiffBundle, CollectError> {
    let raw = run_git(
        repo_dir,
        &[
            "show",
            "--pretty=format:",
            "--no-ext-diff",
            "--text",
            "--unified=3",
            commit,
        ],
        true,
    )?;
    let diff_map: BTreeMap<String, String> = split_unified_diffs(&raw).into_iter().collect();
    let preview = build_preview(&diff_map, rules);
    let changes = to_changes(&preview, &diff_map);
    Ok(LocalDiffBundle {
        mode: "commit".into(),
        repo_dir: repo_dir.display().to_string(),
        changes,
        preview,
        warnings: Vec::new(),
    })
}

pub fn collect_range_diffs(
    repo_dir: &Path,
    from_ref: &str,
    to_ref: &str,
    rules: &RuleResolver,
) -> Result<LocalDiffBundle, CollectError> {
    let mut warnings = Vec::new();
    let base_raw = run_git(repo_dir, &["merge-base", from_ref, to_ref], false)?;
    let base = base_raw.trim();
    let base = if base.is_empty() {
        warnings.push(format!(
            "无法计算 merge-base({from_ref}, {to_ref})，回退直连 {from_ref}..{to_ref}"
        ));
        from_ref.to_string()
    } else {
        base.to_string()
    };
    let range = format!("{base}...{to_ref}");
    let raw = run_git(
        repo_dir,
        &["diff", "--no-ext-diff", "--text", "--unified=3", &range],
        true,
    )?;
    let diff_map: BTreeMap<String, String> = split_unified_diffs(&raw).into_iter().collect();
    let preview = build_preview(&diff_map, rules);
    let changes = to_changes(&preview, &diff_map);
    Ok(LocalDiffBundle {
        mode: "range".into(),
        repo_dir: repo_dir.display().to_string(),
        changes,
        preview,
        warnings,
    })
}

pub fn resolve_git_root(path: &Path) -> PathBuf {
    let cur = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut p = cur.canonicalize().unwrap_or(cur);
    loop {
        if p.join(".git").exists() {
            return p;
        }
        if !p.pop() {
            break;
        }
    }
    path.to_path_buf()
}

fn filter_to_subdir(mut bundle: LocalDiffBundle, subdir: &Path) -> LocalDiffBundle {
    let root = PathBuf::from(&bundle.repo_dir);
    let Ok(root) = root.canonicalize() else {
        return bundle;
    };
    let Ok(target) = subdir.canonicalize() else {
        return bundle;
    };
    let Ok(rel) = target.strip_prefix(&root) else {
        return bundle;
    };
    let rel = rel.to_string_lossy().replace('\\', "/");
    if rel.is_empty() || rel == "." {
        return bundle;
    }
    let prefix = if rel.ends_with('/') {
        rel.clone()
    } else {
        format!("{rel}/")
    };
    let under = |path: &str| {
        let norm = path.replace('\\', "/");
        norm == rel || norm.starts_with(&prefix)
    };
    bundle.preview.retain(|i| under(&i.path));
    bundle.changes.retain(|c| under(&c.new_path));
    if !bundle.warnings.iter().any(|w| w.starts_with("scope=")) {
        bundle
            .warnings
            .push(format!("scope={rel}（仅审查子目录内变更）"));
    }
    bundle
}

/// 入口：workspace / 单 commit / ref range。
pub fn collect_local_diffs(
    repo_dir: impl AsRef<Path>,
    commit: Option<&str>,
    from_ref: Option<&str>,
    to_ref: Option<&str>,
    rules: Option<&RuleResolver>,
) -> Result<LocalDiffBundle, CollectError> {
    let user_path = if repo_dir.as_ref().is_absolute() {
        repo_dir.as_ref().to_path_buf()
    } else {
        std::env::current_dir()?.join(repo_dir.as_ref())
    };
    let git_root = resolve_git_root(&user_path);
    let owned_rules;
    let rules = match rules {
        Some(r) => r,
        None => {
            owned_rules = RuleResolver::default();
            &owned_rules
        }
    };

    let bundle = if let Some(commit) = commit {
        collect_commit_diffs(&git_root, commit, rules)?
    } else if from_ref.is_some() || to_ref.is_some() {
        let (Some(from_ref), Some(to_ref)) = (from_ref, to_ref) else {
            return Err(CollectError::InvalidArgs(
                "--from 与 --to 需同时提供".into(),
            ));
        };
        collect_range_diffs(&git_root, from_ref, to_ref, rules)?
    } else {
        collect_workspace_diffs(&git_root, rules)?
    };

    Ok(filter_to_subdir(bundle, &user_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git_diff::decide_path;
    use std::process::Command;

    #[test]
    fn decide_filters_noise_and_ext() {
        let rules = RuleResolver::new("missing-rules", "missing-allowlist");
        let noise = decide_path("node_modules/a.js", "+x\n", &rules);
        assert!(!noise.included);
        assert_eq!(noise.reason, "noise_dir");

        let bad = decide_path("a.bin", "+x\n", &rules);
        assert!(!bad.included);
        assert_eq!(bad.reason, "unsupported_ext");

        let ok = decide_path("a.py", "+print(1)\n", &rules);
        assert!(ok.included);
    }

    #[test]
    fn collect_workspace_in_temp_repo() {
        let dir = tempfile_dir();
        run(&dir, &["git", "init"]);
        run(&dir, &["git", "config", "user.email", "t@example.com"]);
        run(&dir, &["git", "config", "user.name", "t"]);
        fs::write(dir.join("ok.py"), "print(1)\n").unwrap();
        run(&dir, &["git", "add", "ok.py"]);
        run(&dir, &["git", "commit", "-m", "init"]);
        fs::write(dir.join("ok.py"), "print(2)\n").unwrap();
        fs::write(dir.join("skip.bin"), "x").unwrap();

        let rules = RuleResolver::new("missing-rules", "missing-allowlist");
        let bundle = collect_workspace_diffs(&dir, &rules).unwrap();
        assert_eq!(bundle.mode, "workspace");
        assert!(bundle
            .preview
            .iter()
            .any(|p| p.path == "ok.py" && p.included));
        assert!(bundle
            .preview
            .iter()
            .any(|p| p.path == "skip.bin" && !p.included));
        assert!(bundle.changes.iter().any(|c| c.new_path == "ok.py"));
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "xiezhi-scr-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn run(dir: &Path, args: &[&str]) {
        let status = Command::new(args[0])
            .args(&args[1..])
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "{args:?}");
    }
}
