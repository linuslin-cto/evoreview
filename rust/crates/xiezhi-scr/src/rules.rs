//! 规则解析（对齐 `biz.scr.rules` 的路径过滤与规则文档映射）。

use regex::Regex;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_EXTS: &[&str] = &[
    ".java", ".py", ".php", ".ts", ".tsx", ".js", ".jsx", ".go", ".rs",
];

#[derive(Debug, Clone)]
pub struct RuleResolver {
    pub rules_root: PathBuf,
    pub allowlist_root: PathBuf,
    supported_exts: Vec<String>,
    exclude_patterns: Vec<Regex>,
}

impl Default for RuleResolver {
    fn default() -> Self {
        Self::new("conf/scr/rules", "conf/scr/allowlist")
    }
}

impl RuleResolver {
    pub fn new(rules_root: impl Into<PathBuf>, allowlist_root: impl Into<PathBuf>) -> Self {
        let rules_root = rules_root.into();
        let allowlist_root = allowlist_root.into();
        let supported_exts = load_string_list(allowlist_root.join("supported_file_types.json"))
            .unwrap_or_else(|| {
                std::env::var("SUPPORTED_EXTENSIONS")
                    .ok()
                    .map(|raw| {
                        raw.split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    })
                    .unwrap_or_else(|| DEFAULT_EXTS.iter().map(|s| (*s).to_string()).collect())
            });
        let exclude_patterns =
            load_string_list(allowlist_root.join("default_exclude_patterns.json"))
                .unwrap_or_default()
                .into_iter()
                .flat_map(|p| expand_braces(&p))
                .filter_map(|p| Regex::new(&format!("(?i){}", glob_to_regex(&p))).ok())
                .collect();
        Self {
            rules_root,
            allowlist_root,
            supported_exts,
            exclude_patterns,
        }
    }

    /// 按扩展名映射规则文档名（与 Python rule_docs 对齐的子集）。
    pub fn rule_doc_name_for_path(path: &str) -> &'static str {
        let lower = path.replace('\\', "/").to_ascii_lowercase();
        if lower.contains(".github/workflows/") {
            return "github_workflows";
        }
        if lower.contains(".github/") {
            return "github_config";
        }
        let name = Path::new(&lower)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        match name {
            "cargo.toml" => return "cargo_toml",
            "package.json" => return "package_json",
            "pom.xml" => return "pom_xml",
            "build.gradle" | "build.gradle.kts" => return "build_gradle",
            _ => {}
        }
        let ext = Path::new(&lower)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        match ext {
            "py" => "python",
            "rs" => "rust",
            "java" => "java",
            "kt" | "kts" => "kotlin",
            "c" => "c",
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => "cpp",
            "ts" | "tsx" | "js" | "jsx" => "ts_js_tsx_jsx",
            "json" => "json",
            "yml" | "yaml" => "yaml",
            "properties" => "properties",
            "astro" => "astro",
            "ets" => "arkts",
            "xml" if name.contains("mapper") || name.contains("dao") => "mapper_dao_xml",
            _ => "default",
        }
    }

    pub fn rule_doc_path(&self, path: &str) -> PathBuf {
        self.rules_root
            .join("rule_docs")
            .join(format!("{}.md", Self::rule_doc_name_for_path(path)))
    }

    /// 读取路径对应规则文档；缺失则回退 default.md。
    pub fn resolve(&self, path: &str) -> String {
        let primary = self.rule_doc_path(path);
        if let Ok(text) = fs::read_to_string(&primary) {
            let t = text.trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
        let fallback = self.rules_root.join("rule_docs").join("default.md");
        fs::read_to_string(fallback)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    pub fn is_excluded_by_default(&self, path: &str) -> bool {
        let mut norm = path.replace('\\', "/");
        while let Some(rest) = norm.strip_prefix("./") {
            norm = rest.to_string();
        }
        self.exclude_patterns.iter().any(|rx| rx.is_match(&norm))
    }

    pub fn is_supported_path(&self, path: &str) -> bool {
        let norm = path.replace('\\', "/");
        self.supported_exts.iter().any(|ext| norm.ends_with(ext))
    }
}

fn load_string_list(path: PathBuf) -> Option<Vec<String>> {
    let raw = fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let arr = v.as_array()?;
    Some(
        arr.iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
    )
}

/// 展开一层花括号，如 `*.{ts,js}` -> [`*.ts`, `*.js`]。
pub fn expand_braces(pattern: &str) -> Vec<String> {
    if let Some(start) = pattern.find('{') {
        if let Some(end_rel) = pattern[start..].find('}') {
            let end = start + end_rel;
            let prefix = &pattern[..start];
            let suffix = &pattern[end + 1..];
            let inner = &pattern[start + 1..end];
            let mut results = Vec::new();
            for option in inner.split(',') {
                results.extend(expand_braces(&format!("{prefix}{option}{suffix}")));
            }
            return results;
        }
    }
    vec![pattern.to_string()]
}

/// 把 glob（支持 ** / * / ?）转换为正则，按 POSIX 路径匹配。
pub fn glob_to_regex(pattern: &str) -> String {
    let mut out = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    i += 2;
                    if i < chars.len() && chars[i] == '/' {
                        i += 1;
                        out.push_str("(?:.*/)?");
                    } else {
                        out.push_str(".*");
                    }
                } else {
                    out.push_str("[^/]*");
                    i += 1;
                }
            }
            '?' => {
                out.push_str("[^/]");
                i += 1;
            }
            '.' => {
                out.push_str(r"\.");
                i += 1;
            }
            c => {
                out.push_str(&regex::escape(&c.to_string()));
                i += 1;
            }
        }
    }
    out.push('$');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_common_extensions() {
        assert_eq!(RuleResolver::rule_doc_name_for_path("a.py"), "python");
        assert_eq!(RuleResolver::rule_doc_name_for_path("lib.rs"), "rust");
        assert_eq!(
            RuleResolver::rule_doc_name_for_path("Cargo.toml"),
            "cargo_toml"
        );
        assert_eq!(
            RuleResolver::rule_doc_name_for_path(".github/workflows/ci.yml"),
            "github_workflows"
        );
    }

    #[test]
    fn brace_expand_and_glob() {
        let expanded = expand_braces("**/*.test.{js,ts}");
        assert_eq!(expanded.len(), 2);
        let rx = Regex::new(&format!("(?i){}", glob_to_regex(&expanded[0]))).unwrap();
        assert!(rx.is_match("src/foo.test.js"));
        assert!(!rx.is_match("src/foo.js"));
    }

    #[test]
    fn supported_and_exclude_with_defaults() {
        let r = RuleResolver::new(
            PathBuf::from("missing-rules"),
            PathBuf::from("missing-allowlist"),
        );
        assert!(r.is_supported_path("a.py"));
        assert!(!r.is_supported_path("a.bin"));
    }
}
