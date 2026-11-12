//! LLM 输出 JSON 抽取（对齐 Python `biz.scr.reviewer._extract_json*`）。

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static FENCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)```(?:json)?\s*(.*?)```").unwrap());

fn strip_fence(text: &str) -> String {
    let cleaned = text.trim();
    if let Some(caps) = FENCE_RE.captures(cleaned) {
        caps[1].trim().to_string()
    } else {
        cleaned.to_string()
    }
}

/// 从模型输出中抽取 JSON object。
pub fn extract_json(text: &str) -> Value {
    if text.trim().is_empty() {
        return Value::Object(Default::default());
    }
    let cleaned = strip_fence(text);
    if let Ok(v) = serde_json::from_str::<Value>(&cleaned) {
        if v.is_object() {
            return v;
        }
    }
    if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if end > start {
            if let Ok(v) = serde_json::from_str::<Value>(&cleaned[start..=end]) {
                if v.is_object() {
                    return v;
                }
            }
        }
    }
    Value::Object(Default::default())
}

/// 从模型输出中抽取 JSON array。
pub fn extract_json_array(text: &str) -> Vec<Value> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let cleaned = strip_fence(text);
    if let Ok(v) = serde_json::from_str::<Value>(&cleaned) {
        if let Some(arr) = v.as_array() {
            return arr.clone();
        }
    }
    if let (Some(start), Some(end)) = (cleaned.find('['), cleaned.rfind(']')) {
        if end > start {
            if let Ok(v) = serde_json::from_str::<Value>(&cleaned[start..=end]) {
                if let Some(arr) = v.as_array() {
                    return arr.clone();
                }
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_object_from_fence() {
        let raw = "这里是结果：\n```json\n{\"comments\":[{\"content\":\"x\"}]}\n```\n";
        let v = extract_json(raw);
        assert_eq!(v["comments"][0]["content"], "x");
    }

    #[test]
    fn extract_object_from_noise() {
        let raw = "好的，审查如下 {\"comments\":[]} 谢谢";
        let v = extract_json(raw);
        assert!(v.get("comments").is_some());
    }

    #[test]
    fn extract_array_ids() {
        let raw = "```json\n[\"c-0\", \"c-2\"]\n```";
        let arr = extract_json_array(raw);
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0], "c-0");
    }
}
