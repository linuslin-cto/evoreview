# Rust 迁移进度

目标：用 Rust 重构獬豸核心服务与 CLI，并保持行为与 Python 对齐；每步都有自动化测试。

## 已完成

- [x] Workspace / 域模型 / 评分 / 锚定 / diff 采集 / 报告合成
- [x] OpenAI 兼容 LLM + SCR 结构化审查编排
- [x] Anthropic / Ollama / 智谱客户端
- [x] 叙述总览（diff_only）+ fusion + Agentic（工具沙箱 + LocalRepoSyncer）
- [x] CLI review 入口
- [x] Python↔Rust 黄金样例
- [x] Webhook 入队 + HttpCommentPoster
- [x] Dashboard API + **Reporter LLM**（`/api/reports/summary`；无 Key 时确定性回退）
- [x] Docker 多阶段 + compose `--profile rust`
- [x] **Webhook worker**：`review_fusion` + SQLite/`Memory` 写库（`insert_mr` / `insert_push`）+ 与 Dashboard 共用 `shared_review_store`；GitLab MR changes API；`_xiezhi_changes` / 注入 changes 便于测试；`WORKER_DRY_RUN` 无 Key 回退
- [x] **MR/PR `last_commit` 去重**：`ReviewStore::mr_last_commit_exists`（Memory + SQLite），对齐 `ReviewService.check_mr_last_commit_id_exists`
- [x] **GitHub / Gitea changes 拉取**：PR `/files`、Push `/compare/{before}...{after}`（Gitea `/api/v1/...`）；本地 mock API 单测
- [x] **GitLab Push changes**：`repository/compare?from&to`；新分支走 `commits/{sha}/diff`；删分支跳过
- [x] **Inline comments**：`format_inline_body`；GitLab MR discussions（diff_refs）；GitHub PR review comments；Gitea pull reviews；`SCR_INLINE_COMMENTS_ENABLED`；worker MR/PR 接入
- [x] **受控分支保护检查**：`MERGE_REVIEW_ONLY_PROTECTED_BRANCHES_ENABLED`；GitLab/GitHub/Gitea protected branches API + fnmatch；worker MR/PR 门禁
- [x] **IM notifier 骨架**：`Notifier` + Feishu/WeCom/DingTalk/ExtraWebhook + `CompositeNotifier::from_env`；draft MR / queue 失败时 `send_notification`
- [x] **审查完成 IM markdown 通知**：`im/events` 对齐 `event_manager`（MR/Push 文案 + 标题分数）；worker 在 `insert_mr` / `insert_push` 后 `notify_*`；`MrNotify` / `PushNotify` 避免 clippy `too_many_arguments`
- [x] **每日报告调度器**：`GET /review/daily_report`；`REPORT_CRONTAB_EXPRESSION`（默认 `0 18 * * 1-5`）后台调度；`PUSH_REVIEW_ENABLED` 选择 push/mr 日志；Reporter LLM + fallback；IM「代码提交日报」
- [x] **CLI config init/path**：`~/.xiezhi`（`XIEZHI_USER_HOME`）；复制 `.env` / `scr` / `prompt_templates.yml`；`review` 前 `ensure_runtime_paths`
- [x] **钉钉 Webhook HMAC 签名**：`DINGTALK_SECRET_ENABLED` + `DINGTALK_SECRET`；`sign_dingtalk_url` 与 Python `_sign_url` 对齐
- [x] **首页 HTML + `/assets`**：对齐 `home.py`；`XIEZHI_ASSETS_DIR` / cwd / 仓库根解析；`tower-http` ServeDir
- [x] CI + clippy/fmt/test 门禁

- [x] **CLI menu + 专项审查**：`xiezhi menu`；`directory` / `branch` / `complexity` 子命令（复杂度为启发式，非 lizard）
- [x] **MySQL 表结构审查**：`xiezhi mysql` + menu 选项 5；对齐 `MySQLReviewFunc`（SHOW TABLES / FULL COLUMNS / PK / FK → LLM）
- [x] **Harness² 自进化（首轮）**：`xiezhi-review/harness2` Propose–Probe–Compose；`HARNESS2_ENABLED` / library；fusion 后处理探针；详见 `rust/HARNESS2.md`

## 状态

**Rust 重构与自动化门禁已完成**（tick 27）。Harness² 自进化 loop 已完成（见 `.loop/harness2-state.json`、`rust/HARNESS2.md`）。

## 行为对齐约定

1. 评分与 Python 黄金样例精确相等。
2. 报告标题 / footer `> xiezhi <version>` 一致。
3. Dashboard Reporter 提示词与 Python `Reporter.generate_report` 对齐；未配置 LLM 时返回 deterministic fallback，配置后失败返回 500。
4. Worker 在草稿 / 非目标分支 / 非关注 action 时跳过；空 changes 回帖「关注的文件没有修改」并仍写库。
