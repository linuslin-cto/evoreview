//! 惰性 clone / fetch+checkout（对齐 `biz.agent.repo_syncer.LocalRepoSyncer`）。

use regex::Regex;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, warn};
use url::Url;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Git(String),
}

#[derive(Debug, Clone)]
pub struct RepoSyncSpec {
    pub url: String,
    pub key: String,
    pub git_ref: String,
    pub cache_root: PathBuf,
}

pub struct LocalRepoSyncer {
    cache_root: PathBuf,
    clone_timeout_secs: u64,
    lock_wait_secs: u64,
}

impl LocalRepoSyncer {
    pub fn new(cache_root: impl AsRef<Path>) -> Self {
        Self {
            cache_root: cache_root.as_ref().to_path_buf(),
            clone_timeout_secs: 300,
            lock_wait_secs: 60,
        }
    }

    pub fn with_timeouts(mut self, clone_timeout_secs: u64, lock_wait_secs: u64) -> Self {
        self.clone_timeout_secs = clone_timeout_secs;
        self.lock_wait_secs = lock_wait_secs;
        self
    }

    /// 确保本地有该仓库并 checkout 到 ref，返回本地路径。
    pub fn sync_to(&self, url: &str, key: &str, git_ref: &str) -> Result<PathBuf, SyncError> {
        fs::create_dir_all(&self.cache_root)?;
        let safe = sanitize_key(key);
        let target = self.cache_root.join(&safe);
        let lock_path = self.cache_root.join(format!("{safe}.lock"));

        let _lock = FileLock::acquire(&lock_path, self.lock_wait_secs)?;

        if !target.join(".git").exists() {
            self.clone(&auth_url(url), &target)?;
        }
        self.ensure_authenticated_remote(&target)?;
        self.fetch_and_checkout(&target, git_ref)?;
        Ok(target)
    }

    fn clone(&self, url: &str, target: &Path) -> Result<(), SyncError> {
        debug!(url = %redact_url(url), target = %target.display(), "cloning");
        let out = run_git(
            &["clone", url, &target.display().to_string()],
            None,
            self.clone_timeout_secs,
        )?;
        if !out.status.success() {
            return Err(SyncError::Git(format!(
                "git clone failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    }

    fn ensure_authenticated_remote(&self, target: &Path) -> Result<(), SyncError> {
        let out = match run_git(&["remote", "get-url", "origin"], Some(target), 30) {
            Ok(o) if o.status.success() => o,
            _ => return Ok(()),
        };
        let existing = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let new_url = auth_url(&existing);
        if new_url == existing {
            return Ok(());
        }
        let out = run_git(&["remote", "set-url", "origin", &new_url], Some(target), 30)?;
        if !out.status.success() {
            return Err(SyncError::Git(format!(
                "git remote set-url failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    }

    fn fetch_and_checkout(&self, target: &Path, git_ref: &str) -> Result<(), SyncError> {
        let out = run_git(
            &["fetch", "--all", "--prune"],
            Some(target),
            self.clone_timeout_secs,
        )?;
        if !out.status.success() {
            return Err(SyncError::Git(format!(
                "git fetch failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let is_sha = Regex::new(r"^[0-9a-fA-F]{7,40}$")
            .expect("sha regex")
            .is_match(git_ref);
        let target_ref = if is_sha {
            git_ref.to_string()
        } else {
            format!("origin/{git_ref}")
        };
        let out = run_git(
            &["reset", "--hard", &target_ref],
            Some(target),
            self.clone_timeout_secs,
        )?;
        if !out.status.success() {
            return Err(SyncError::Git(format!(
                "git checkout failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(())
    }
}

pub fn sanitize_key(key: &str) -> String {
    let re = Regex::new(r"[^A-Za-z0-9._-]").expect("sanitize regex");
    re.replace_all(key, "_").into_owned()
}

pub fn pick_token_for_host(host: Option<&str>) -> Option<String> {
    let host = host?.to_ascii_lowercase();
    if host == "github.com" || host.ends_with(".github.com") {
        return std::env::var("GITHUB_ACCESS_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());
    }
    if host.contains("gitea") {
        return std::env::var("GITEA_ACCESS_TOKEN")
            .ok()
            .filter(|s| !s.is_empty());
    }
    std::env::var("GITLAB_ACCESS_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn auth_url(url: &str) -> String {
    let Ok(parsed) = Url::parse(url) else {
        return url.to_string();
    };
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return url.to_string();
    }
    if parsed.username() != "" || parsed.password().is_some() {
        warn!("URL already contains credentials; skipping token injection");
        return url.to_string();
    }
    let Some(host) = parsed.host_str() else {
        return url.to_string();
    };
    let Some(token) = pick_token_for_host(Some(host)) else {
        return url.to_string();
    };
    let mut out = parsed;
    let _ = out.set_username("oauth2");
    let _ = out.set_password(Some(&token));
    out.to_string()
}

fn redact_url(url: &str) -> String {
    if let Ok(mut u) = Url::parse(url) {
        if u.password().is_some() {
            let _ = u.set_password(Some("***"));
        }
        return u.to_string();
    }
    url.to_string()
}

fn run_git(
    args: &[&str],
    cwd: Option<&Path>,
    _timeout_secs: u64,
) -> Result<std::process::Output, SyncError> {
    // Windows/CI：依赖 OS 调度；超时留给后续迭代（spawn + kill）。
    let mut cmd = Command::new("git");
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.output().map_err(SyncError::Io)
}

struct FileLock {
    path: Option<PathBuf>,
    _file: Option<File>,
}

impl FileLock {
    fn acquire(path: &Path, wait_secs: u64) -> Result<Self, SyncError> {
        let deadline = Instant::now() + Duration::from_secs(wait_secs);
        loop {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(f) => {
                    return Ok(Self {
                        path: Some(path.to_path_buf()),
                        _file: Some(f),
                    });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    if Instant::now() >= deadline {
                        warn!(
                            lock = %path.display(),
                            wait_secs,
                            "could not acquire lock, proceeding without lock"
                        );
                        return Ok(Self {
                            path: None,
                            _file: None,
                        });
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => return Err(SyncError::Io(e)),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(cwd: &Path, args: &[&str]) {
        let st = Command::new("git")
            .args(args)
            .current_dir(cwd)
            .status()
            .unwrap();
        assert!(st.success(), "git {args:?}");
    }

    #[test]
    fn sanitize_replaces_slash() {
        assert_eq!(sanitize_key("group/proj"), "group_proj");
    }

    #[test]
    fn auth_url_injects_gitlab_token() {
        std::env::set_var("GITLAB_ACCESS_TOKEN", "glpat-test");
        let out = auth_url("https://gitlab.example.com/g/p.git");
        assert!(out.contains("oauth2:glpat-test@"));
        std::env::remove_var("GITLAB_ACCESS_TOKEN");
    }

    #[test]
    fn sync_clones_and_checks_out_branch() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("xiezhi-sync-{stamp}"));
        let remote = root.join("remote.git");
        let work = root.join("work");
        let cache = root.join("cache");
        fs::create_dir_all(&work).unwrap();

        // bare remote via clone --bare from a seeded worktree
        git(&work, &["init", "-b", "main"]);
        git(&work, &["config", "user.email", "t@example.com"]);
        git(&work, &["config", "user.name", "t"]);
        fs::write(work.join("a.txt"), "one\n").unwrap();
        git(&work, &["add", "a.txt"]);
        git(&work, &["commit", "-m", "c1"]);
        git(&work, &["checkout", "-b", "feature"]);
        fs::write(work.join("a.txt"), "two\n").unwrap();
        git(&work, &["add", "a.txt"]);
        git(&work, &["commit", "-m", "c2"]);
        git(&work, &["checkout", "main"]);

        let st = Command::new("git")
            .args([
                "clone",
                "--bare",
                work.to_str().unwrap(),
                remote.to_str().unwrap(),
            ])
            .status()
            .unwrap();
        assert!(st.success());

        let syncer = LocalRepoSyncer::new(&cache).with_timeouts(60, 5);
        let local = syncer
            .sync_to(
                &format!(
                    "file:///{}",
                    remote.display().to_string().replace('\\', "/")
                ),
                "demo/repo",
                "feature",
            )
            .unwrap();
        assert!(local.join(".git").exists());
        let content = fs::read_to_string(local.join("a.txt")).unwrap();
        assert_eq!(content.replace("\r\n", "\n"), "two\n");

        // second sync should reuse clone
        let local2 = syncer
            .sync_to(
                &format!(
                    "file:///{}",
                    remote.display().to_string().replace('\\', "/")
                ),
                "demo/repo",
                "main",
            )
            .unwrap();
        assert_eq!(local, local2);
        let content = fs::read_to_string(local2.join("a.txt")).unwrap();
        assert_eq!(content.replace("\r\n", "\n"), "one\n");

        let _ = fs::remove_dir_all(root);
    }
}
