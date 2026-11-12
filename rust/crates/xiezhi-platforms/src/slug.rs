//! URL slugify（对齐 Python `slugify_url`）。

use regex::Regex;
use std::sync::LazyLock;

static SCHEME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^https?://").unwrap());
static NON_ALNUM_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-zA-Z0-9]").unwrap());

/// Turn a URL into a filesystem-safe slug.
pub fn slugify_url(original_url: &str) -> String {
    let without_scheme = SCHEME_RE.replace(original_url, "");
    let slug = NON_ALNUM_RE.replace_all(&without_scheme, "_");
    slug.trim_end_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_basic() {
        assert_eq!(
            slugify_url("https://gitlab.example.com/"),
            "gitlab_example_com"
        );
        assert_eq!(slugify_url("https://github.com"), "github_com");
    }
}
