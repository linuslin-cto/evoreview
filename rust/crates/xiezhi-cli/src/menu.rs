//! 交互式菜单（对齐 `biz.cmd.review`）。

use crate::{branch, complexity, directory, mysql_review, prompt};
use anyhow::{bail, Result};
use std::path::PathBuf;
use xiezhi_llm::factory_from_env;
use xiezhi_review::{review_fusion, FusionOptions, StructuredOptions};
use xiezhi_scr::{collect_local_diffs, RuleResolver};

pub async fn run_menu() -> Result<()> {
    prompt::welcome();
    match prompt::choose_menu()? {
        1 => run_diff_interactive().await?,
        2 => directory::run_interactive().await?,
        3 => branch::run_interactive().await?,
        4 => complexity::run_interactive()?,
        5 => mysql_review::run_interactive().await?,
        _ => bail!("unreachable"),
    }
    Ok(())
}

async fn run_diff_interactive() -> Result<()> {
    let repo = prompt::get_user_input("请输入 Git 仓库路径", ".")?;
    let mode = prompt::get_user_input("模式: 1=工作区(默认) 2=commit 3=范围(from..to)", "1")?;
    let preview_only = prompt::confirm_action("是否仅预览过滤结果（不调用 LLM）? (y/n): ")?;

    let mut commit = None;
    let mut from_ref = None;
    let mut to_ref = None;
    if mode == "2" {
        commit = Some(prompt::get_user_input("请输入 commit SHA/引用", "HEAD")?);
    } else if mode == "3" {
        from_ref = Some(prompt::get_user_input("请输入 --from", "origin/main")?);
        to_ref = Some(prompt::get_user_input("请输入 --to", "HEAD")?);
    }

    let rules = RuleResolver::default();
    let path = PathBuf::from(&repo);
    let bundle = match collect_local_diffs(
        &path,
        commit.as_deref(),
        from_ref.as_deref(),
        to_ref.as_deref(),
        Some(&rules),
    ) {
        Ok(b) => b,
        Err(e) => {
            println!("❌ 采集 Diff 失败: {e}");
            return Ok(());
        }
    };

    let included: Vec<_> = bundle.preview.iter().filter(|i| i.included).collect();
    let excluded: Vec<_> = bundle.preview.iter().filter(|i| !i.included).collect();
    println!(
        "\n模式={} · 纳入={} · 排除={}",
        bundle.mode,
        included.len(),
        excluded.len()
    );
    for i in &included {
        println!("  ✅ {} (+{}/-{})", i.path, i.additions, i.deletions);
    }
    for i in excluded.iter().take(20) {
        println!("  ⏭️ {} ({})", i.path, i.reason);
    }
    if excluded.len() > 20 {
        println!("  ... 其余 {} 个已排除", excluded.len() - 20);
    }

    if preview_only {
        println!("\n（预览模式，未调用 LLM）");
        return Ok(());
    }
    if bundle.changes.is_empty() {
        println!("没有可审查的代码变更。");
        return Ok(());
    }
    if !prompt::confirm_action(&format!(
        "确认对 {} 个文件发起审查? (y/n): ",
        bundle.changes.len()
    ))? {
        return Ok(());
    }

    let narrative = prompt::confirm_action("是否附加叙述性总览（会多一次 LLM 调用）? (y/n): ")?;
    println!("\n⏳ 审查中...");

    let structured =
        StructuredOptions::from_search_roots(&[path.clone(), PathBuf::from(&bundle.repo_dir)]);
    let mut fusion = FusionOptions::from_structured(structured);
    fusion.repo_root = Some(PathBuf::from(&bundle.repo_dir));
    fusion.narrative_enabled = narrative;

    let client = factory_from_env()?;
    let outcome = review_fusion(client.as_ref(), &bundle.changes, &rules, &fusion).await?;
    println!("\n{}", outcome.report_text);
    Ok(())
}
