//! 首页与静态资源（对齐 `biz.api.routes.home`）。

use axum::response::{Html, IntoResponse};
use axum::Router;
use std::env;
use std::path::PathBuf;
use tower_http::services::ServeDir;
use tracing::info;
use xiezhi_core::VERSION;

const HOME_HTML: &str = r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8" />
  <title>獬豸</title>
  <link rel="icon" href="/assets/xiezhi-logo.png" type="image/png" />
  <style>
    body { font-family: sans-serif; text-align: center; margin-top: 4rem; color: #1a2332; }
    img { width: 120px; height: 120px; border-radius: 20px; object-fit: cover; }
  </style>
</head>
<body>
  <img src="/assets/xiezhi-logo.png" alt="獬豸" />
  <h2>獬豸 API 服务运行中。</h2>
  <p style="color:#667;font-size:0.9rem">xiezhi {VERSION} (Rust)</p>
</body>
</html>
"#;

pub fn home_html() -> Html<String> {
    Html(HOME_HTML.replace("{VERSION}", VERSION))
}

pub async fn home_handler() -> impl IntoResponse {
    home_html()
}

/// 解析 `assets/` 目录：`XIEZHI_ASSETS_DIR` → cwd/assets → 可执行文件旁 → 仓库根（开发）。
pub fn resolve_assets_dir() -> PathBuf {
    if let Ok(p) = env::var("XIEZHI_ASSETS_DIR") {
        let path = PathBuf::from(p.trim());
        if path.is_dir() {
            return path;
        }
    }
    let mut candidates = Vec::new();
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("assets"));
        for parent in cwd.ancestors().skip(1).take(4) {
            candidates.push(parent.join("assets"));
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("assets"));
            candidates.push(parent.join("..").join("assets"));
        }
    }
    if let Ok(manifest) = env::var("CARGO_MANIFEST_DIR") {
        let crate_dir = PathBuf::from(manifest);
        if let Some(ws) = crate_dir.parent().and_then(|p| p.parent()) {
            candidates.push(ws.join("assets"));
            candidates.push(ws.parent().unwrap_or(ws).join("assets"));
        }
    }
    for c in &candidates {
        if c.is_dir() {
            return c.clone();
        }
    }
    PathBuf::from("assets")
}

pub fn assets_router() -> Router {
    let dir = resolve_assets_dir();
    info!(path = %dir.display(), "serving /assets");
    Router::new().nest_service("/assets", ServeDir::new(dir))
}

/// 路径穿越防护：仅允许落在 assets 根下的相对文件。
#[cfg(test)]
mod tests {
    use super::*;

    fn safe_asset_path(assets_root: &std::path::Path, filename: &str) -> Option<PathBuf> {
        if filename.is_empty() || filename.contains('\0') {
            return None;
        }
        let requested = std::path::Path::new(filename);
        if requested.is_absolute() {
            return None;
        }
        let joined = assets_root.join(requested);
        let root = assets_root.canonicalize().ok()?;
        let canon = joined.canonicalize().ok()?;
        if canon.starts_with(&root) && canon.is_file() {
            Some(canon)
        } else {
            None
        }
    }

    #[test]
    fn home_mentions_brand() {
        let html = home_html().0;
        assert!(html.contains("獬豸"));
        assert!(html.contains("/assets/xiezhi-logo.png"));
        assert!(html.contains(VERSION));
    }

    #[test]
    fn safe_path_rejects_escape() {
        let tmp = env::temp_dir().join(format!(
            "xiezhi-assets-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(tmp.join("ok.png"), b"x").unwrap();
        assert!(safe_asset_path(&tmp, "ok.png").is_some());
        assert!(safe_asset_path(&tmp, "../ok.png").is_none());
        assert!(safe_asset_path(&tmp, "/etc/passwd").is_none());
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
