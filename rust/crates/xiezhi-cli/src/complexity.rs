//! 代码复杂度审查（对齐 `ComplexityReviewFunc`；轻量启发式，不依赖 lizard）。

use crate::prompt::{get_user_input, get_user_input_int};
use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FuncComplexity {
    pub name: String,
    pub filename: String,
    pub complexity: u32,
}

const CODE_EXTS: &[&str] = &[
    "rs", "py", "js", "jsx", "ts", "tsx", "java", "kt", "go", "c", "cpp", "h", "hpp", "php", "cs",
];

fn is_code_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| CODE_EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// 粗略圈复杂度：1 + 决策关键字计数。
pub fn estimate_file_functions(path: &Path, source: &str) -> Vec<FuncComplexity> {
    let mut out = Vec::new();
    let file = path.display().to_string();
    let mut current_name = format!(
        "<module:{}>",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("?")
    );
    let mut complexity: u32 = 1;
    let mut in_func = false;

    for line in source.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with("//") || t.starts_with('#') {
            continue;
        }
        if let Some(name) = detect_func_start(t) {
            if in_func {
                out.push(FuncComplexity {
                    name: current_name.clone(),
                    filename: file.clone(),
                    complexity,
                });
            }
            current_name = name;
            complexity = 1;
            in_func = true;
        }
        complexity += decision_points(t);
    }
    if in_func || complexity > 1 {
        out.push(FuncComplexity {
            name: current_name,
            filename: file,
            complexity,
        });
    }
    out
}

fn detect_func_start(line: &str) -> Option<String> {
    if line.starts_with("fn ") || line.starts_with("pub fn ") || line.starts_with("async fn ") {
        return Some(extract_ident(line, &["fn"]));
    }
    if line.starts_with("def ") {
        return Some(extract_ident(line, &["def"]));
    }
    if line.starts_with("function ") || line.contains("function(") {
        return Some(extract_ident(line, &["function"]));
    }
    if line.contains("func ") && (line.contains('(') || line.contains('{')) {
        return Some(extract_ident(line, &["func"]));
    }
    if (line.contains(" public ")
        || line.starts_with("public ")
        || line.starts_with("private ")
        || line.starts_with("protected "))
        && line.contains('(')
        && !line.contains('=')
    {
        return Some(extract_ident(
            line,
            &["public", "private", "protected", "static", "final", "void"],
        ));
    }
    None
}

fn extract_ident(line: &str, skip: &[&str]) -> String {
    let parts = line
        .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
        .filter(|s| !s.is_empty());
    for p in parts {
        if skip.iter().any(|s| s.eq_ignore_ascii_case(p)) {
            continue;
        }
        return p.to_string();
    }
    "anonymous".into()
}

fn decision_points(line: &str) -> u32 {
    let padded = format!(" {} ", line.trim());
    let keys = [
        " if ", " if(", " elif ", " else if", " elseif", " for ", " for(", " while ", " while(",
        " case ", " catch ", " && ", " || ", " match ", " when ",
    ];
    let mut n = 0u32;
    for k in keys {
        n += padded.matches(k).count() as u32;
    }
    // 三元 ? 但不计类型 `?`
    if padded.contains('?') && !padded.contains("?Sized") {
        n += padded.matches('?').count() as u32;
    }
    n
}

pub fn find_most_complex(directory: &Path, top_n: usize) -> Result<Vec<FuncComplexity>> {
    let mut all = Vec::new();
    walk(directory, &mut all)?;
    all.sort_by_key(|a| std::cmp::Reverse(a.complexity));
    all.truncate(top_n);
    Ok(all)
}

fn walk(dir: &Path, out: &mut Vec<FuncComplexity>) -> Result<()> {
    let rd = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => bail!("分析目录失败: {e}"),
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "node_modules" || name == "target" || name == "vendor" {
            continue;
        }
        if path.is_dir() {
            walk(&path, out)?;
        } else if is_code_file(&path) {
            if let Ok(src) = fs::read_to_string(&path) {
                out.extend(estimate_file_functions(&path, &src));
            }
        }
    }
    Ok(())
}

pub fn run_interactive() -> Result<()> {
    let directory = loop {
        let d = get_user_input("请输入项目的根目录路径", ".")?;
        if PathBuf::from(&d).is_dir() {
            break d;
        }
        println!("❌ 目录不存在，请输入有效路径");
    };
    let top_n = get_user_input_int("请输入需要分析的复杂度最高文件数量", 10)?;
    if top_n <= 0 {
        bail!("请输入正整数");
    }
    run_with_args(&directory, top_n as usize)
}

pub fn run_with_args(directory: &str, top_n: usize) -> Result<()> {
    let root = PathBuf::from(directory);
    if !root.is_dir() {
        bail!("目录不存在: {directory}");
    }
    let top = find_most_complex(&root, top_n)?;
    println!("🔥 以下是最复杂的函数：");
    for func in top {
        println!(
            "{} (文件: {}, 复杂度: {})",
            func.name, func.filename, func.complexity
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_if_branches() {
        let src = r#"
fn foo() {
    if a { }
    else if b { }
    for x in y { }
}
"#;
        let funcs = estimate_file_functions(Path::new("t.rs"), src);
        assert!(!funcs.is_empty());
        assert!(funcs[0].complexity >= 3, "{:?}", funcs[0]);
    }
}
