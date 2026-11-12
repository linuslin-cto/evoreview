//! 交互式输入辅助（对齐 `BaseReviewFunc`）。

use anyhow::Result;
use std::io::{self, BufRead, Write};

pub fn welcome() {
    println!("\n欢迎使用 Codebase Review 工具！\n");
}

pub fn read_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

pub fn get_user_input(prompt: &str, default: &str) -> Result<String> {
    let line = read_line(&format!("{prompt} (默认: {default}): "))?;
    if line.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(line)
    }
}

pub fn get_user_input_int(prompt: &str, default: i64) -> Result<i64> {
    loop {
        let line = read_line(&format!("{prompt} (默认: {default}): "))?;
        if line.is_empty() {
            return Ok(default);
        }
        match line.parse::<i64>() {
            Ok(v) => return Ok(v),
            Err(_) => println!("输入无效，请输入整数。"),
        }
    }
}

pub fn confirm_action(prompt: &str) -> Result<bool> {
    loop {
        let line = read_line(prompt)?.to_ascii_lowercase();
        match line.as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("请输入 'y' 或 'n' 确认。"),
        }
    }
}

pub fn choose_menu() -> Result<u8> {
    println!("📌 请选择功能:");
    println!("1. 本地 Diff 审查（提交前自审 / commit / 范围）");
    println!("2. Review 目录结构规范");
    println!("3. Review 代码分支命名规范");
    println!("4. Review 代码复杂度");
    println!("5. Review MySQL 数据库表结构");
    loop {
        let choice = read_line("请输入数字 (1-5): ")?;
        match choice.as_str() {
            "1" | "2" | "3" | "4" | "5" => return Ok(choice.parse().unwrap()),
            _ => println!("❌ 无效的选择，请输入 1-5"),
        }
    }
}

pub fn strip_markdown_fence(text: &str) -> String {
    let t = text.trim();
    if t.starts_with("```markdown") && t.ends_with("```") {
        t.trim_start_matches("```markdown")
            .trim_end_matches("```")
            .trim()
            .to_string()
    } else if t.starts_with("```") && t.ends_with("```") {
        let inner = t.trim_start_matches('`').trim_end_matches('`');
        let inner = inner
            .strip_prefix("markdown")
            .unwrap_or(inner)
            .trim_start_matches('\n');
        inner.trim().to_string()
    } else {
        t.to_string()
    }
}

pub fn require_non_empty(prompt: &str) -> Result<String> {
    loop {
        let v = read_line(prompt)?;
        if !v.is_empty() {
            return Ok(v);
        }
        println!("❌ 该项不能为空");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_fence() {
        let s = strip_markdown_fence("```markdown\nhello\n```");
        assert_eq!(s, "hello");
        assert_eq!(strip_markdown_fence("plain"), "plain");
    }
}
