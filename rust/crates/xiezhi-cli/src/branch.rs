//! GitLab 分支命名审查（对齐 `BranchReviewFunc`）。

use crate::prompt::{confirm_action, require_non_empty, strip_markdown_fence};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::env;
use url::Url;
use xiezhi_llm::{factory_from_env, ChatMessage, ChatRequest, LlmClient};

const SYSTEM_PROMPT: &str = r#"你是一位资深的软件架构师，本次任务是对一个代码库的分支名称进行审查，具体要求如下：

### 具体要求：
1.命名规范一致性：分支命名是否符合团队或项目定义的命名规范；是否遵循统一的命名风格（如小写字母、连字符分隔等）。
2.分支命名的可读性：分支名称是否能够清晰地表达其目的或功能；是否使用了容易引起混淆的缩写或术语。
3.分支命名的功能性：分支名称是否能够准确描述其实现的功能或修复的问题；是否与相关的任务、需求或问题（如 Jira Issue、GitLab Issue 等）关联。
4.分支命名的唯一性：分支名称是否与现有分支重复或过于相似；对于临时分支，是否包含时间戳或唯一标识以避免冲突。
5.分支命名的长度：分支名称是否过长，导致难以阅读或操作；是否在描述清晰的前提下尽量保持简短。
6.分支命名的类型化：是否使用了合适的前缀来区分分支类型；分支类型是否与其实际用途一致；

### 输出格式：
1.请按照以下格式输出review结果：
2.优点：列出目录结构的优点。
3.潜在问题：指出目录结构中可能存在的问题。
4.改进建议：提供具体的优化建议。"#;

pub fn parse_gitlab_url(url: &str) -> Result<(String, String)> {
    let parsed = Url::parse(url).context("无效的 GitLab 项目 URL")?;
    if parsed.scheme().is_empty() || parsed.host_str().is_none() {
        bail!("❌ 无效的 GitLab 项目 URL，请确保 URL 格式正确");
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("❌ 无效的 GitLab 项目 URL，请确保 URL 格式正确"))?;
    let gitlab_url = match parsed.port() {
        Some(p) => format!("{}://{host}:{p}", parsed.scheme()),
        None => format!("{}://{host}", parsed.scheme()),
    };
    let path = parsed.path().trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let parts: Vec<_> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() < 2 {
        bail!("❌ 无法提取 GitLab 项目路径，请检查 URL 是否正确");
    }
    let project_id = format!("{}/{}", parts[0], parts[1]);
    Ok((gitlab_url, project_id))
}

fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return token.to_string();
    }
    format!("{}****{}", &token[..4], &token[token.len() - 4..])
}

pub async fn fetch_branch_names(gitlab_url: &str, project_id: &str, token: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .build()
        .context("http client")?;
    let encoded = urlencoding::encode(project_id);
    let mut page = 1u32;
    let mut names = Vec::new();
    loop {
        let url = format!(
            "{gitlab_url}/api/v4/projects/{encoded}/repository/branches?per_page=100&page={page}"
        );
        let resp = client
            .get(&url)
            .header("PRIVATE-TOKEN", token)
            .send()
            .await
            .context("gitlab branches")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("GitLab branches HTTP {status}: {text}");
        }
        let batch: Vec<Value> = resp.json().await.context("parse branches")?;
        if batch.is_empty() {
            break;
        }
        for b in &batch {
            if let Some(n) = b.get("name").and_then(|v| v.as_str()) {
                names.push(n.to_string());
            }
        }
        if batch.len() < 100 {
            break;
        }
        page += 1;
        if page > 50 {
            break;
        }
    }
    Ok(names.join("\n"))
}

pub async fn review_branches(client: &dyn LlmClient, branch_list: &str) -> Result<String> {
    let user =
        format!("以下是一个代码库的分支列表，请对其进行审查，并给出详细的评价。\n\n{branch_list}");
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
    let (gitlab_url, project_id) = loop {
        let url = require_non_empty(
            "请输入 GitLab 项目的完整 URL (例如 https://gitlab.example.com/root/test): ",
        )?;
        match parse_gitlab_url(&url) {
            Ok(v) => break v,
            Err(e) => println!("{e}"),
        }
    };
    let mut token = env::var("GITLAB_ACCESS_TOKEN").unwrap_or_default();
    if token.is_empty() {
        token = require_non_empty("请输入 GitLab 访问令牌: ")?;
    }
    run_with_args(&gitlab_url, &project_id, &token, false, false).await
}

pub async fn run_with_args(
    gitlab_url: &str,
    project_id: &str,
    token: &str,
    dry_run: bool,
    skip_confirm: bool,
) -> Result<()> {
    println!("gitlab_url={gitlab_url}");
    println!("access_token={}", mask_token(token));
    println!("project_id={project_id}");
    let branch_names = fetch_branch_names(gitlab_url, project_id, token).await?;
    println!("分支列表:\n{branch_names}");
    if dry_run {
        println!("\n（dry-run：未调用 LLM）");
        return Ok(());
    }
    if !skip_confirm && !confirm_action("是否确认发送 Review 请求？(y/n): ")? {
        println!("用户取消操作，退出程序。");
        return Ok(());
    }
    let client = factory_from_env().context("LLM factory")?;
    let result = review_branches(client.as_ref(), &branch_names).await?;
    println!("Review 结果:\n{result}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_url() {
        let (base, id) = parse_gitlab_url("https://gitlab.example.com/root/test.git").unwrap();
        assert_eq!(base, "https://gitlab.example.com");
        assert_eq!(id, "root/test");
    }
}
