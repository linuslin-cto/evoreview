export type ReviewItem = {
  project_name?: string;
  author?: string;
  source_branch?: string;
  target_branch?: string;
  branch?: string;
  updated_at?: string;
  commit_messages?: string;
  delta?: string;
  score?: number;
  url?: string;
  review_result?: string;
  additions?: number;
  deletions?: number;
};

export type NamedValue = { name: string; value: number };
export type LineValue = { name: string; additions: number; deletions: number };
export type DailyTrend = { date: string; count: number; avg_score: number };
export type LowAuthor = { name: string; count: number; avg: number };

export type ReviewStats = {
  total: number;
  avg_score: number;
  low_score_count: number;
  authors: number;
  projects: number;
  additions: number;
  deletions: number;
  score_threshold: number;
  by_project_count: NamedValue[];
  by_project_score: NamedValue[];
  by_author_count: NamedValue[];
  by_author_score: NamedValue[];
  by_author_lines: LineValue[];
  by_project_lines: LineValue[];
  score_distribution: NamedValue[];
  daily_trend: DailyTrend[];
  low_authors: LowAuthor[];
};

export type MemberSummary = {
  author: string;
  review_count: number;
  avg_score: number;
  min_score: number;
  max_score: number;
  low_count: number;
  additions: number;
  deletions: number;
  project_count: number;
};

export type SystemInfo = {
  llm_provider: string;
  review_strategy: string;
  review_style: string;
  push_review_enabled: boolean;
  protected_branches_only: boolean;
  supported_extensions: string;
  review_max_tokens: string;
  repo_cache_dir: string;
  score_threshold: number;
  report_crontab: string;
  tokens: { gitlab: string; github: string; gitea: string };
  notify: {
    dingtalk: boolean;
    wecom: boolean;
    feishu: boolean;
    extra_webhook: boolean;
  };
  log_tail: string;
};
