//! CLI 配置目录解析（对齐 `biz.cli.paths`）。

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn home_dir() -> PathBuf {
    if let Ok(override_home) = env::var("XIEZHI_USER_HOME") {
        let trimmed = override_home.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    dirs_home().join(".xiezhi")
}

fn dirs_home() -> PathBuf {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn is_conf_dir(p: &Path) -> bool {
    p.join("scr").is_dir() && p.join("prompt_templates.yml").is_file()
}

fn iter_search_bases() -> Vec<PathBuf> {
    let mut bases = Vec::new();
    bases.push(home_dir());
    if let Ok(cwd) = env::current_dir() {
        bases.push(cwd.clone());
        for parent in cwd.ancestors().skip(1) {
            bases.push(parent.to_path_buf());
        }
    }
    // 从 crate/workspace 旁路：可执行文件旁、当前二进制相对路径
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            bases.push(parent.to_path_buf());
        }
    }
    // 开发时：仓库根（含 conf/）
    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let crate_dir = PathBuf::from(manifest);
        if let Some(ws) = crate_dir.parent().and_then(|p| p.parent()) {
            bases.push(ws.to_path_buf());
        }
        bases.push(crate_dir);
    }

    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    for b in bases {
        let key = b.canonicalize().unwrap_or(b.clone());
        if seen.insert(key) {
            ordered.push(b);
        }
    }
    ordered
}

pub fn find_asset_conf() -> Option<PathBuf> {
    if let Ok(env_conf) = env::var("XIEZHI_CONF_DIR").or_else(|_| env::var("XIEZHI_HOME")) {
        if !env_conf.trim().is_empty() {
            let p = PathBuf::from(&env_conf);
            if is_conf_dir(&p) {
                return Some(p);
            }
            let nested = p.join("conf");
            if is_conf_dir(&nested) {
                return Some(nested);
            }
        }
    }
    for base in iter_search_bases() {
        if is_conf_dir(&base) {
            return Some(base);
        }
        let conf = base.join("conf");
        if is_conf_dir(&conf) {
            return Some(conf);
        }
    }
    None
}

pub fn find_env_file() -> Option<PathBuf> {
    let mut candidates = vec![
        home_dir().join(".env"),
        home_dir().join("conf").join(".env"),
    ];
    if let Some(asset) = find_asset_conf() {
        candidates.push(asset.join(".env"));
        if let Some(parent) = asset.parent() {
            candidates.push(parent.join(".env"));
        }
    }
    candidates.into_iter().find(|c| c.is_file())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).with_context(|| format!("mkdir {}", dst.display()))?;
    for entry in fs::read_dir(src).with_context(|| format!("read_dir {}", src.display()))? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), &to)
                .with_context(|| format!("copy {} -> {}", entry.path().display(), to.display()))?;
        }
    }
    Ok(())
}

pub fn init_user_home(force: bool) -> Result<PathBuf> {
    let home = home_dir();
    fs::create_dir_all(&home).with_context(|| format!("mkdir {}", home.display()))?;
    let asset = find_asset_conf().ok_or_else(|| {
        anyhow::anyhow!(
            "未找到内置/旁路配置资产，无法初始化用户目录。\n\
             请使用完整发布包（含 conf/），或设置 XIEZHI_HOME。"
        )
    })?;

    let dest_env = home.join(".env");
    let mut src_env = asset.join(".env.dist");
    if !src_env.is_file() {
        src_env = asset.join(".env");
    }
    if src_env.is_file() && (force || !dest_env.exists()) {
        fs::copy(&src_env, &dest_env)
            .with_context(|| format!("copy {} -> {}", src_env.display(), dest_env.display()))?;
    }

    let dest_scr = home.join("scr");
    if force || !dest_scr.exists() {
        if dest_scr.exists() {
            fs::remove_dir_all(&dest_scr)
                .with_context(|| format!("remove {}", dest_scr.display()))?;
        }
        copy_dir_recursive(&asset.join("scr"), &dest_scr)?;
    }

    let prompt = asset.join("prompt_templates.yml");
    let dest_prompt = home.join("prompt_templates.yml");
    if prompt.is_file() && (force || !dest_prompt.exists()) {
        fs::copy(&prompt, &dest_prompt)
            .with_context(|| format!("copy {} -> {}", prompt.display(), dest_prompt.display()))?;
    }

    Ok(home)
}

pub fn ensure_runtime_paths() -> Result<PathBuf> {
    let mut conf = find_asset_conf().ok_or_else(|| {
        anyhow::anyhow!(
            "未找到獬豸配置目录（需要 scr/ 与 prompt_templates.yml）。\n\
             请使用完整发布包，或执行：\n\
               xiezhi config init\n\
             亦可设置：\n\
               XIEZHI_HOME=/path/to/release   # 含 conf/ 的目录\n\
               XIEZHI_CONF_DIR=/path/to/conf"
        )
    })?;

    let home = home_dir();
    if is_conf_dir(&home) {
        conf = home;
    }

    set_default_env("SCR_RULES_DIR", &conf.join("scr").join("rules"));
    set_default_env("SCR_ALLOWLIST_DIR", &conf.join("scr").join("allowlist"));
    set_default_env(
        "XIEZHI_PROMPT_TEMPLATES",
        &conf.join("prompt_templates.yml"),
    );
    set_default_env("XIEZHI_CONF_DIR", &conf);

    if let Some(env_file) = find_env_file() {
        let _ = dotenvy::from_path(&env_file);
    }
    Ok(conf)
}

fn set_default_env(key: &str, path: &Path) {
    if env::var_os(key).is_none() {
        env::set_var(key, path.as_os_str());
    }
}

pub fn print_paths() {
    println!("user_home={}", home_dir().display());
    match find_env_file() {
        Some(p) => println!("env_file={}", p.display()),
        None => println!("env_file=(未找到)"),
    }
    match find_asset_conf() {
        Some(p) => println!("asset_conf={}", p.display()),
        None => println!("asset_conf=(未找到)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn write_mini_conf(root: &Path) {
        fs::create_dir_all(root.join("scr").join("rules")).unwrap();
        fs::write(root.join("prompt_templates.yml"), "templates: {}\n").unwrap();
        fs::write(root.join(".env.dist"), "LLM_PROVIDER=openai\n").unwrap();
        fs::create_dir_all(root.join("scr").join("allowlist")).unwrap();
        fs::write(root.join("scr").join("allowlist").join("x.json"), "{}").unwrap();
    }

    #[test]
    fn init_and_path_roundtrip() {
        let _g = ENV_LOCK.lock().unwrap();
        let tmp = tempfile_dir();
        let asset = tmp.join("asset_conf");
        let home = tmp.join("user_home");
        write_mini_conf(&asset);

        env::set_var("XIEZHI_CONF_DIR", &asset);
        env::set_var("XIEZHI_USER_HOME", &home);

        assert!(find_asset_conf().unwrap().ends_with("asset_conf") || is_conf_dir(&asset));
        let got = init_user_home(false).unwrap();
        assert_eq!(got, home_dir());
        assert!(home.join(".env").is_file());
        assert!(home.join("scr").join("allowlist").join("x.json").is_file());
        assert!(home.join("prompt_templates.yml").is_file());

        // second init without force keeps existing
        let env_before = fs::read_to_string(home.join(".env")).unwrap();
        fs::write(home.join(".env"), "KEEP=1\n").unwrap();
        init_user_home(false).unwrap();
        assert_eq!(fs::read_to_string(home.join(".env")).unwrap(), "KEEP=1\n");
        let _ = env_before;

        init_user_home(true).unwrap();
        assert!(fs::read_to_string(home.join(".env"))
            .unwrap()
            .contains("LLM_PROVIDER"));

        env::remove_var("XIEZHI_CONF_DIR");
        env::remove_var("XIEZHI_USER_HOME");
    }

    fn tempfile_dir() -> PathBuf {
        let p = env::temp_dir().join(format!(
            "xiezhi-cli-paths-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
}
