//! 审查日志存储（SQLite / 内存）。

use rusqlite::{params_from_iter, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewKind {
    Mr,
    Push,
}

#[derive(Debug, Clone, Default)]
pub struct ReviewLog {
    pub id: Option<i64>,
    pub project_name: Option<String>,
    pub author: Option<String>,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
    pub branch: Option<String>,
    pub updated_at: Option<i64>,
    pub commit_messages: Option<String>,
    pub score: Option<i64>,
    pub url: Option<String>,
    pub review_result: Option<String>,
    pub additions: Option<i64>,
    pub deletions: Option<i64>,
    pub last_commit_id: Option<String>,
}

pub trait ReviewStore: Send + Sync {
    fn query(
        &self,
        kind: ReviewKind,
        authors: Option<&[String]>,
        projects: Option<&[String]>,
        updated_at_gte: Option<i64>,
        updated_at_lte: Option<i64>,
    ) -> Vec<ReviewLog>;

    fn insert_mr(&self, log: ReviewLog) -> Result<(), String>;
    fn insert_push(&self, log: ReviewLog) -> Result<(), String>;

    /// 对齐 `ReviewService.check_mr_last_commit_id_exists`。
    fn mr_last_commit_exists(
        &self,
        project_name: &str,
        source_branch: &str,
        target_branch: &str,
        last_commit_id: &str,
    ) -> bool;
}

#[derive(Default)]
pub struct MemoryReviewStore {
    pub mr: Mutex<Vec<ReviewLog>>,
    pub push: Mutex<Vec<ReviewLog>>,
}

impl MemoryReviewStore {
    pub fn insert_mr_mut(&mut self, log: ReviewLog) {
        self.mr.get_mut().unwrap().push(log);
    }
}

impl ReviewStore for MemoryReviewStore {
    fn query(
        &self,
        kind: ReviewKind,
        authors: Option<&[String]>,
        projects: Option<&[String]>,
        updated_at_gte: Option<i64>,
        updated_at_lte: Option<i64>,
    ) -> Vec<ReviewLog> {
        let guard = match kind {
            ReviewKind::Mr => self.mr.lock().unwrap(),
            ReviewKind::Push => self.push.lock().unwrap(),
        };
        guard
            .iter()
            .filter(|r| {
                if let Some(authors) = authors {
                    let a = r.author.as_deref().unwrap_or("");
                    if !authors.iter().any(|x| x == a) {
                        return false;
                    }
                }
                if let Some(projects) = projects {
                    let p = r.project_name.as_deref().unwrap_or("");
                    if !projects.iter().any(|x| x == p) {
                        return false;
                    }
                }
                if let Some(gte) = updated_at_gte {
                    if r.updated_at.unwrap_or(0) < gte {
                        return false;
                    }
                }
                if let Some(lte) = updated_at_lte {
                    if r.updated_at.unwrap_or(0) > lte {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    fn insert_mr(&self, log: ReviewLog) -> Result<(), String> {
        self.mr.lock().map_err(|e| e.to_string())?.push(log);
        Ok(())
    }

    fn insert_push(&self, log: ReviewLog) -> Result<(), String> {
        self.push.lock().map_err(|e| e.to_string())?.push(log);
        Ok(())
    }

    fn mr_last_commit_exists(
        &self,
        project_name: &str,
        source_branch: &str,
        target_branch: &str,
        last_commit_id: &str,
    ) -> bool {
        let Ok(guard) = self.mr.lock() else {
            return false;
        };
        guard.iter().any(|r| {
            r.project_name.as_deref() == Some(project_name)
                && r.source_branch.as_deref() == Some(source_branch)
                && r.target_branch.as_deref() == Some(target_branch)
                && r.last_commit_id.as_deref() == Some(last_commit_id)
        })
    }
}

pub struct SqliteReviewStore {
    path: String,
}

impl SqliteReviewStore {
    #[allow(dead_code)]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(format!("db not found: {}", path.display()));
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        let _ = conn
            .query_row("SELECT 1", [], |r| r.get::<_, i64>(0))
            .optional()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            path: path.display().to_string(),
        })
    }

    pub fn open_or_create(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS mr_review_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_name TEXT,
                author TEXT,
                source_branch TEXT,
                target_branch TEXT,
                updated_at INTEGER,
                commit_messages TEXT,
                score INTEGER,
                url TEXT,
                review_result TEXT,
                additions INTEGER DEFAULT 0,
                deletions INTEGER DEFAULT 0,
                last_commit_id TEXT DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS push_review_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_name TEXT,
                author TEXT,
                branch TEXT,
                updated_at INTEGER,
                commit_messages TEXT,
                score INTEGER,
                review_result TEXT,
                additions INTEGER DEFAULT 0,
                deletions INTEGER DEFAULT 0
            );
            "#,
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            path: path.display().to_string(),
        })
    }

    fn connect(&self) -> Result<Connection, String> {
        Connection::open(&self.path).map_err(|e| e.to_string())
    }
}

impl ReviewStore for SqliteReviewStore {
    fn query(
        &self,
        kind: ReviewKind,
        authors: Option<&[String]>,
        projects: Option<&[String]>,
        updated_at_gte: Option<i64>,
        updated_at_lte: Option<i64>,
    ) -> Vec<ReviewLog> {
        let Ok(conn) = self.connect() else {
            return Vec::new();
        };
        let (table, select) = match kind {
            ReviewKind::Mr => (
                "mr_review_log",
                "id, project_name, author, source_branch, target_branch, NULL as branch, updated_at, commit_messages, score, url, review_result, additions, deletions, last_commit_id",
            ),
            ReviewKind::Push => (
                "push_review_log",
                "id, project_name, author, NULL as source_branch, NULL as target_branch, branch, updated_at, commit_messages, score, NULL as url, review_result, additions, deletions, NULL as last_commit_id",
            ),
        };

        let mut sql = format!("SELECT {select} FROM {table} WHERE 1=1");
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(authors) = authors {
            if !authors.is_empty() {
                let placeholders = vec!["?"; authors.len()].join(",");
                sql.push_str(&format!(" AND author IN ({placeholders})"));
                for a in authors {
                    params.push(Box::new(a.clone()));
                }
            }
        }
        if let Some(projects) = projects {
            if !projects.is_empty() {
                let placeholders = vec!["?"; projects.len()].join(",");
                sql.push_str(&format!(" AND project_name IN ({placeholders})"));
                for p in projects {
                    params.push(Box::new(p.clone()));
                }
            }
        }
        if let Some(gte) = updated_at_gte {
            sql.push_str(" AND updated_at >= ?");
            params.push(Box::new(gte));
        }
        if let Some(lte) = updated_at_lte {
            sql.push_str(" AND updated_at <= ?");
            params.push(Box::new(lte));
        }
        sql.push_str(" ORDER BY updated_at DESC");

        let mut stmt = match conn.prepare(&sql) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_from_iter(param_refs), |row| {
            Ok(ReviewLog {
                id: row.get(0)?,
                project_name: row.get(1)?,
                author: row.get(2)?,
                source_branch: row.get(3)?,
                target_branch: row.get(4)?,
                branch: row.get(5)?,
                updated_at: row.get(6)?,
                commit_messages: row.get(7)?,
                score: row.get(8)?,
                url: row.get(9)?,
                review_result: row.get(10)?,
                additions: row.get(11)?,
                deletions: row.get(12)?,
                last_commit_id: row.get(13)?,
            })
        });
        match rows {
            Ok(iter) => iter.filter_map(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn insert_mr(&self, log: ReviewLog) -> Result<(), String> {
        let conn = self.connect()?;
        conn.execute(
            r#"INSERT INTO mr_review_log (
                project_name, author, source_branch, target_branch, updated_at,
                commit_messages, score, url, review_result, additions, deletions, last_commit_id
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"#,
            rusqlite::params![
                log.project_name,
                log.author,
                log.source_branch,
                log.target_branch,
                log.updated_at,
                log.commit_messages,
                log.score,
                log.url,
                log.review_result,
                log.additions.unwrap_or(0),
                log.deletions.unwrap_or(0),
                log.last_commit_id.unwrap_or_default(),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn insert_push(&self, log: ReviewLog) -> Result<(), String> {
        let conn = self.connect()?;
        conn.execute(
            r#"INSERT INTO push_review_log (
                project_name, author, branch, updated_at, commit_messages,
                score, review_result, additions, deletions
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)"#,
            rusqlite::params![
                log.project_name,
                log.author,
                log.branch,
                log.updated_at,
                log.commit_messages,
                log.score,
                log.review_result,
                log.additions.unwrap_or(0),
                log.deletions.unwrap_or(0),
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn mr_last_commit_exists(
        &self,
        project_name: &str,
        source_branch: &str,
        target_branch: &str,
        last_commit_id: &str,
    ) -> bool {
        let Ok(conn) = self.connect() else {
            return false;
        };
        conn.query_row(
            r#"SELECT COUNT(*) FROM mr_review_log
               WHERE project_name = ?1 AND source_branch = ?2
                 AND target_branch = ?3 AND last_commit_id = ?4"#,
            rusqlite::params![project_name, source_branch, target_branch, last_commit_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false)
    }
}
