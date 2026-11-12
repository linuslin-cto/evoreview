//! Dashboard / 日报摘要 Reporter（对齐 `biz.utils.reporter.Reporter`）。

use xiezhi_llm::{ChatMessage, ChatRequest, LlmClient, LlmError};

const REPORT_PREFIX: &str = "下面是以json格式记录员工代码提交信息。请总结这些信息，生成每个员工的工作日报摘要。员工姓名直接用json内容中的author属性值，不要进行转换。特别要求:以Markdown格式返回。\n";

pub fn build_report_user_prompt(data_json: &str) -> String {
    format!("{REPORT_PREFIX}{data_json}")
}

/// 调用 LLM 生成 Markdown 日报摘要。
pub async fn generate_report(client: &dyn LlmClient, data_json: &str) -> Result<String, LlmError> {
    let content = build_report_user_prompt(data_json);
    client
        .chat(ChatRequest {
            messages: vec![ChatMessage::user(content)],
            ..Default::default()
        })
        .await
}

/// 无 LLM 时的确定性回退摘要（保持 API 可用）。
pub fn fallback_report(records_json: &str, record_count: usize) -> String {
    format!("## 审查摘要\n\n共 {record_count} 条记录。\n\n```json\n{records_json}\n```\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use xiezhi_llm::ScriptedClient;

    #[test]
    fn prompt_mentions_author_and_markdown() {
        let p = build_report_user_prompt(r#"[{"author":"alice"}]"#);
        assert!(p.contains("author"));
        assert!(p.contains("Markdown"));
        assert!(p.contains("alice"));
    }

    #[tokio::test]
    async fn generate_uses_llm() {
        let client = ScriptedClient::new("s", ["## 日报\n- alice: 完成 MR".into()]);
        let out = generate_report(&client, r#"[{"author":"alice","score":90}]"#)
            .await
            .unwrap();
        assert!(out.contains("alice"));
    }
}
