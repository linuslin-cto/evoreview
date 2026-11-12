//! 獬豸 CLI 入口：`xiezhi <subcommand>`。

mod branch;
mod complexity;
mod directory;
mod menu;
mod mysql_review;
mod paths;
mod prompt;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use xiezhi_core::VERSION;
use xiezhi_llm::{factory_from_env, ScriptedClient};
use xiezhi_review::{review_changes_to_outcome, review_fusion, FusionOptions, StructuredOptions};
use xiezhi_scr::{collect_local_diffs, RuleResolver};

#[derive(Parser, Debug)]
#[command(
    name = "xiezhi",
    version = VERSION,
    about = "獬豸 AI 代码审查 CLI —— 本地 Diff 审查（Rust）"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 显示版本号
    Version,
    /// 交互式功能菜单
    Menu,
    /// 配置管理（init / path）
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
    /// 本地 Diff 审查
    Review {
        #[arg(long)]
        preview: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_narrative: bool,
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        commit: Option<String>,
        #[arg(long = "from")]
        from_ref: Option<String>,
        #[arg(long = "to")]
        to_ref: Option<String>,
    },
    /// 目录结构规范审查
    Directory {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value = "python")]
        language: String,
        #[arg(long, default_value_t = 3)]
        max_depth: usize,
        #[arg(long, default_value_t = true)]
        only_dirs: bool,
        #[arg(long)]
        dry_run: bool,
        /// 跳过确认直接审查
        #[arg(long)]
        yes: bool,
    },
    /// GitLab 分支命名审查
    Branch {
        /// GitLab 项目完整 URL
        #[arg(long)]
        url: String,
        #[arg(long, env = "GITLAB_ACCESS_TOKEN")]
        token: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
    /// 代码复杂度 Top-N（启发式）
    Complexity {
        #[arg(long, default_value = ".")]
        path: PathBuf,
        #[arg(long, default_value_t = 10)]
        top: usize,
    },
    /// MySQL 表结构审查
    Mysql {
        #[arg(long, default_value = "localhost")]
        host: String,
        #[arg(long, default_value_t = 3306)]
        port: u16,
        #[arg(long, default_value = "root")]
        user: String,
        #[arg(long)]
        password: Option<String>,
        #[arg(long)]
        database: String,
        /// 表名 LIKE 通配符；省略则全部表
        #[arg(long)]
        pattern: Option<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    Init {
        #[arg(long)]
        force: bool,
    },
    Path,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let _ = dotenvy::from_filename("conf/.env");

    let cli = Cli::parse();
    match cli.command {
        Commands::Version => {
            println!("xiezhi {VERSION}");
        }
        Commands::Menu => {
            let _ = paths::ensure_runtime_paths().context("resolve config paths")?;
            menu::run_menu().await?;
        }
        Commands::Config { action } => match action.unwrap_or(ConfigAction::Path) {
            ConfigAction::Path => paths::print_paths(),
            ConfigAction::Init { force } => {
                let home = paths::init_user_home(force)?;
                println!("已初始化用户配置目录: {}", home.display());
                println!("请编辑密钥文件: {}", home.join(".env").display());
                println!("至少填写 LLM_PROVIDER 与对应 API Key（如 DEEPSEEK_API_KEY）。");
            }
        },
        Commands::Review {
            preview,
            dry_run,
            no_narrative,
            path,
            commit,
            from_ref,
            to_ref,
        } => {
            let _ = paths::ensure_runtime_paths().context("resolve config paths")?;
            run_review(
                preview,
                dry_run,
                no_narrative,
                path,
                commit,
                from_ref,
                to_ref,
            )
            .await?;
        }
        Commands::Directory {
            path,
            language,
            max_depth,
            only_dirs,
            dry_run,
            yes,
        } => {
            let _ = paths::ensure_runtime_paths().ok();
            directory::run_with_args(
                &path.to_string_lossy(),
                &language,
                max_depth,
                only_dirs,
                dry_run,
                yes,
            )
            .await?;
        }
        Commands::Branch {
            url,
            token,
            dry_run,
            yes,
        } => {
            let _ = paths::ensure_runtime_paths().ok();
            let (gitlab_url, project_id) = branch::parse_gitlab_url(&url)?;
            let token = token
                .or_else(|| std::env::var("GITLAB_ACCESS_TOKEN").ok())
                .filter(|s| !s.is_empty())
                .context("需要 --token 或 GITLAB_ACCESS_TOKEN")?;
            branch::run_with_args(&gitlab_url, &project_id, &token, dry_run, yes).await?;
        }
        Commands::Complexity { path, top } => {
            complexity::run_with_args(&path.to_string_lossy(), top)?;
        }
        Commands::Mysql {
            host,
            port,
            user,
            password,
            database,
            pattern,
            dry_run,
            yes,
        } => {
            let _ = paths::ensure_runtime_paths().ok();
            let password = match password {
                Some(p) => p,
                None => std::env::var("MYSQL_PASSWORD").unwrap_or_default(),
            };
            if password.is_empty() {
                bail!("需要 --password 或环境变量 MYSQL_PASSWORD");
            }
            mysql_review::run_with_args(
                mysql_review::MysqlConnectArgs {
                    host,
                    port,
                    user,
                    password,
                    database,
                    pattern,
                },
                dry_run,
                yes,
            )
            .await?;
        }
    }
    Ok(())
}

async fn run_review(
    preview: bool,
    dry_run: bool,
    no_narrative: bool,
    path: PathBuf,
    commit: Option<String>,
    from_ref: Option<String>,
    to_ref: Option<String>,
) -> Result<()> {
    let rules = RuleResolver::default();
    let bundle = collect_local_diffs(
        &path,
        commit.as_deref(),
        from_ref.as_deref(),
        to_ref.as_deref(),
        Some(&rules),
    )
    .context("collect local diffs")?;

    if preview {
        println!(
            "mode={} repo={} files={} included={}",
            bundle.mode,
            bundle.repo_dir,
            bundle.preview.len(),
            bundle.changes.len()
        );
        for w in &bundle.warnings {
            println!("warning: {w}");
        }
        for item in &bundle.preview {
            if item.included {
                println!(
                    "+ {path}  +{a}/-{d}",
                    path = item.path,
                    a = item.additions,
                    d = item.deletions
                );
            } else {
                println!(
                    "- {path}  ({reason})",
                    path = item.path,
                    reason = item.reason
                );
            }
        }
        return Ok(());
    }

    if bundle.changes.is_empty() {
        bail!("没有可审查的变更文件（可用 --preview 查看过滤原因）");
    }

    let structured =
        StructuredOptions::from_search_roots(&[path.clone(), PathBuf::from(&bundle.repo_dir)]);
    let mut fusion = FusionOptions::from_structured(structured.clone());
    fusion.repo_root = Some(PathBuf::from(&bundle.repo_dir));
    if no_narrative {
        fusion.narrative_enabled = false;
    }
    let outcome = if dry_run {
        let mut scripted_responses: Vec<String> = bundle
            .changes
            .iter()
            .map(|_| r#"{"comments":[]}"#.to_string())
            .collect();
        if fusion.narrative_enabled {
            scripted_responses.push("dry-run 叙述总览\n总分:100分".into());
        }
        let scripted = ScriptedClient::new("dry-run", scripted_responses);
        if fusion.narrative_enabled {
            review_fusion(&scripted, &bundle.changes, &rules, &fusion).await?
        } else {
            review_changes_to_outcome(&scripted, &bundle.changes, &rules, &fusion.structured)
                .await?
        }
    } else {
        let client = factory_from_env().context("LLM factory")?;
        review_fusion(client.as_ref(), &bundle.changes, &rules, &fusion).await?
    };

    println!("{}", outcome.report_text);
    eprintln!(
        "score={} inline={} files={}",
        outcome.score,
        outcome.inline_comments.len(),
        bundle.changes.len()
    );
    Ok(())
}
