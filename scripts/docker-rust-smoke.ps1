# 校验 Rust Docker 相关文件存在且 Dockerfile 语法可读（不强制完整 build）。
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$required = @(
    "Dockerfile.rust",
    "rust/Dockerfile",
    "conf/supervisord.rust.conf",
    "docker-compose.yml",
    "rust/Cargo.toml",
    "rust/Cargo.lock"
)
foreach ($rel in $required) {
    $p = Join-Path $root $rel
    if (-not (Test-Path $p)) { throw "missing: $rel" }
    Write-Host "ok $rel"
}

$compose = Get-Content (Join-Path $root "docker-compose.yml") -Raw
if ($compose -notmatch "Dockerfile\.rust") { throw "compose missing Dockerfile.rust" }
if ($compose -notmatch "profiles:\s*\[`"rust`"\]") { throw "compose missing rust profile" }

$df = Get-Content (Join-Path $root "Dockerfile.rust") -Raw
foreach ($needle in @("FROM rust:", "cargo build --release", "xiezhi-server", "supervisord.rust.conf")) {
    if ($df -notmatch [regex]::Escape($needle)) { throw "Dockerfile.rust missing: $needle" }
}

Write-Host "docker-rust-smoke: passed"
if (Get-Command docker -ErrorAction SilentlyContinue) {
    Write-Host "docker available — optional: docker build -f Dockerfile.rust -t xiezhi-rust:dev ."
} else {
    Write-Host "docker CLI not found; file-level smoke only"
}
