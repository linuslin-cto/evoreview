//! 目录结构审查（对齐 `DirectoryReviewFunc`）。

use crate::prompt::{confirm_action, get_user_input, get_user_input_int, strip_markdown_fence};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use xiezhi_llm::{factory_from_env, ChatMessage, ChatRequest, LlmClient};

pub const SUPPORTED_LANGUAGES: &[&str] = &["python", "java", "php", "vue"];

const SYSTEM_PROMPT: &str = r#"你是一位资深的软件架构师，本次任务是对一个代码库进行审查，具体要求如下：
### 具体要求：
1.组织逻辑：评估目录结构是否清晰，是否符合常见的项目组织规范（如MVC、分层架构等）。
2.命名规范性：检查目录和文件的命名是否清晰、一致，是否符合命名约定（如小写字母、短横线分隔等）。
3.模块化程度：评估代码是否按功能或模块合理划分，是否存在过度耦合或冗余。
4.可维护性：分析目录结构是否易于扩展和维护，是否适合团队协作。
5.改进建议：针对发现的问题，提出具体的优化建议。

### 输出格式：
1.请按照以下格式输出review结果：
2.优点：列出目录结构的优点。
3.潜在问题：指出目录结构中可能存在的问题。
4.改进建议：提供具体的优化建议。"#;

pub fn load_gitignore_patterns(root: &Path) -> Vec<String> {
    let path = root.join(".gitignore");
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

fn ignored(rel: &str, patterns: &[String]) -> bool {
    let rel = rel.replace('\\', "/");
    for p in patterns {
        let pat = p.trim_end_matches('/');
        if rel == pat || rel.starts_with(&format!("{pat}/")) {
            return true;
        }
        // 简单 * 后缀：如 `node_modules/` 已在上方；`*.log`
        if let Some(suf) = pat.strip_prefix('*') {
            if rel.ends_with(suf) {
                return true;
            }
        }
    }
    false
}

pub fn directory_tree(
    directory: &Path,
    ignore: &[String],
    max_depth: usize,
    only_dirs: bool,
) -> String {
    #[allow(clippy::too_many_arguments)]
    fn walk(
        dir: &Path,
        root: &Path,
        ignore: &[String],
        max_depth: usize,
        depth: usize,
        prefix: &str,
        only_dirs: bool,
        out: &mut Vec<String>,
    ) {
        if depth >= max_depth {
            return;
        }
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<_> = rd.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        entries.retain(|e| {
            let name = e.file_name();
            let s = name.to_string_lossy();
            !s.starts_with('.')
        });
        if only_dirs {
            entries.retain(|e| e.path().is_dir());
        }
        let len = entries.len();
        for (index, entry) in entries.into_iter().enumerate() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let mut rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if path.is_dir() {
                rel.push('/');
            }
            if ignored(&rel, ignore) {
                continue;
            }
            let is_last = index + 1 == len;
            let connector = if is_last { "└── " } else { "├── " };
            out.push(format!("{prefix}{connector}{name}"));
            if path.is_dir() {
                let new_prefix = format!("{prefix}{}", if is_last { "    " } else { "│   " });
                walk(
                    &path,
                    root,
                    ignore,
                    max_depth,
                    depth + 1,
                    &new_prefix,
                    only_dirs,
                    out,
                );
            }
        }
    }

    let root = directory
        .canonicalize()
        .unwrap_or_else(|_| directory.to_path_buf());
    let mut lines = Vec::new();
    walk(
        &root, &root, ignore, max_depth, 0, "", only_dirs, &mut lines,
    );
    lines.join("\n")
}

pub async fn review_directory_text(
    client: &dyn LlmClient,
    language: &str,
    tree: &str,
) -> Result<String> {
    if tree.trim().is_empty() {
        bail!("目录结构为空，无法审查");
    }
    let user = format!(
        "以下是一个 {language} 代码库，请对其进行审查，并给出详细的评价。\n\n目录结构：\n{tree}"
    );
    let raw = client
        .chat(ChatRequest {
            messages: vec![ChatMessage::system(SYSTEM_PROMPT), ChatMessage::user(user)],
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(strip_markdown_fence(&raw))
}

pub async fn run_interactive() -> Result<()> {
    println!("请选择开发语言:");
    for (i, lang) in SUPPORTED_LANGUAGES.iter().enumerate() {
        println!("{}. {lang}", i + 1);
    }
    let language = loop {
        let choice = get_user_input(
            &format!("请输入数字 (1-{})", SUPPORTED_LANGUAGES.len()),
            "1",
        )?;
        if let Ok(n) = choice.parse::<usize>() {
            if (1..=SUPPORTED_LANGUAGES.len()).contains(&n) {
                break SUPPORTED_LANGUAGES[n - 1].to_string();
            }
        }
        println!("❌ 无效的选择");
    };

    let directory = loop {
        let d = get_user_input("请输入代码项目的根目录路径", ".")?;
        if PathBuf::from(&d).is_dir() {
            break d;
        }
        println!("❌ 目录不存在，请输入有效路径");
    };
    let max_depth = get_user_input_int("请输入目录树的最大深度", 3)? as usize;
    let only_dirs = {
        let v = get_user_input("是否仅获取目录？(y/n)", "y")?;
        matches!(v.to_ascii_lowercase().as_str(), "y" | "yes")
    };

    run_with_args(&directory, &language, max_depth, only_dirs, false, false).await
}

pub async fn run_with_args(
    directory: &str,
    language: &str,
    max_depth: usize,
    only_dirs: bool,
    dry_run: bool,
    skip_confirm: bool,
) -> Result<()> {
    let root = PathBuf::from(directory);
    if !root.is_dir() {
        bail!("目录不存在: {directory}");
    }
    if !SUPPORTED_LANGUAGES.contains(&language) {
        bail!(
            "不支持的语言 {language}，可选: {}",
            SUPPORTED_LANGUAGES.join(", ")
        );
    }
    let ignore = load_gitignore_patterns(&root);
    let tree = directory_tree(&root, &ignore, max_depth, only_dirs);
    println!("目录结构:\n{tree}");
    if tree.trim().is_empty() {
        println!("⚠️ 目录结构为空，跳过 Review");
        return Ok(());
    }
    if dry_run {
        println!("\n（dry-run：未调用 LLM）");
        return Ok(());
    }
    if !skip_confirm && !confirm_action("是否确认发送 Review 请求？(y/n): ")? {
        println!("用户取消操作，退出程序。");
        return Ok(());
    }
    let client = factory_from_env().context("LLM factory")?;
    let result = review_directory_text(client.as_ref(), language, &tree).await?;
    println!("Review 结果:\n{result}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn tree_lists_dirs() {
        let tmp = std::env::temp_dir().join(format!(
            "xiezhi-dir-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(tmp.join("src").join("a")).unwrap();
        fs::write(tmp.join("src").join("a").join("m.py"), "x").unwrap();
        fs::create_dir_all(tmp.join("node_modules")).unwrap();
        let ignore = vec!["node_modules/".into()];
        let tree = directory_tree(&tmp, &ignore, 3, true);
        assert!(tree.contains("src"), "{tree}");
        assert!(!tree.contains("node_modules"), "{tree}");
        let _ = fs::remove_dir_all(&tmp);
    }
}
