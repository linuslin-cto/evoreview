//! MySQL 表结构审查（对齐 `MySQLReviewFunc`）。

use crate::prompt::{
    confirm_action, get_user_input, get_user_input_int, require_non_empty, strip_markdown_fence,
};
use anyhow::{bail, Context, Result};
use mysql::prelude::*;
use mysql::{OptsBuilder, Pool};
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use xiezhi_llm::{factory_from_env, ChatMessage, ChatRequest, LlmClient};

const SYSTEM_PROMPT: &str = r#"你是一个经验丰富的数据库架构专家，擅长MySQL数据库设计评审和性能优化。你在评审数据库结构时会从以下角度进行系统性的分析并提出可操作的建议：
1. 表结构设计（命名规范、主键设置、字段设计等）
2. 字段类型是否合理（如金额使用DECIMAL、避免TEXT/BLOB滥用等）
3. 索引设计（是否有必要索引、冗余索引、联合索引顺序等）
4. 表与表之间的关系建模（是否正确使用外键、中间表、多对多等）
5. 性能与容量规划（是否考虑数据量增长、归档、冷热数据等）
6. 安全性（是否有敏感数据加密、访问控制）
7. 可维护性与扩展性（命名规范、注释、迁移工具支持等）
如果发现潜在问题，请指出原因，并建议最佳实践或优化方式。"#;

fn safe_table_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_$]+$").expect("regex"))
}

pub fn is_safe_table_name(name: &str) -> bool {
    safe_table_re().is_match(name)
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub col_type: String,
    pub primary_key: bool,
    pub foreign_key: Option<ForeignKeyRef>,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ForeignKeyRef {
    pub table: String,
    pub column: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct SchemaDump {
    pub tables: Vec<TableInfo>,
}

#[derive(Debug, Clone)]
pub struct MysqlConnectArgs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    pub pattern: Option<String>,
}

pub fn fetch_mysql_schema(args: &MysqlConnectArgs) -> Result<SchemaDump> {
    let opts = OptsBuilder::new()
        .ip_or_hostname(Some(args.host.clone()))
        .tcp_port(args.port)
        .user(Some(args.user.clone()))
        .pass(Some(args.password.clone()))
        .db_name(Some(args.database.clone()));
    let pool = Pool::new(opts).context("connect mysql")?;
    let mut conn = pool.get_conn().context("mysql get_conn")?;

    let tables: Vec<String> = if let Some(pattern) = args.pattern.as_deref() {
        conn.exec_map("SHOW TABLES LIKE ?", (pattern,), |name: String| name)
            .context("SHOW TABLES LIKE")?
    } else {
        conn.query_map("SHOW TABLES", |name: String| name)
            .context("SHOW TABLES")?
    };

    if tables.is_empty() {
        println!("⚠️ 未匹配到任何表，请检查通配符或数据库是否正确。");
        return Ok(SchemaDump::default());
    }

    let mut schema = SchemaDump::default();
    for table in tables {
        if !is_safe_table_name(&table) {
            println!("⚠️ 跳过非法表名: {table:?}");
            continue;
        }

        // Field, Type, Collation, Null, Key, Default, Extra, Privileges, Comment
        let columns_info: Vec<(String, String, String)> = conn
            .query_map(
                format!("SHOW FULL COLUMNS FROM `{table}`"),
                |(field, ty, _coll, _null, _key, _def, _extra, _priv, comment): (
                    String,
                    String,
                    Option<String>,
                    String,
                    String,
                    Option<String>,
                    String,
                    String,
                    String,
                )| { (field, ty, comment) },
            )
            .with_context(|| format!("SHOW FULL COLUMNS {table}"))?;

        let primary_keys: HashSet<String> = conn
            .exec_map(
                r#"SELECT COLUMN_NAME
                   FROM information_schema.KEY_COLUMN_USAGE
                   WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND CONSTRAINT_NAME = 'PRIMARY'"#,
                (&args.database, &table),
                |name: String| name,
            )
            .context("primary keys")?
            .into_iter()
            .collect();

        let mut foreign_keys: HashMap<String, ForeignKeyRef> = HashMap::new();
        let fks: Vec<(String, String, String)> = conn
            .exec_map(
                r#"SELECT COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME
                   FROM information_schema.KEY_COLUMN_USAGE
                   WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND REFERENCED_TABLE_NAME IS NOT NULL"#,
                (&args.database, &table),
                |(col, rt, rc): (String, String, String)| (col, rt, rc),
            )
            .context("foreign keys")?;
        for (col, rt, rc) in fks {
            foreign_keys.insert(
                col,
                ForeignKeyRef {
                    table: rt,
                    column: rc,
                },
            );
        }

        let mut table_dict = TableInfo {
            name: table,
            columns: Vec::new(),
        };
        for (col_name, col_type, col_comment) in columns_info {
            let is_primary = primary_keys.contains(&col_name);
            let foreign_key = foreign_keys.get(&col_name).cloned();
            table_dict.columns.push(ColumnInfo {
                name: col_name,
                col_type,
                primary_key: is_primary,
                foreign_key,
                comment: col_comment,
            });
        }
        schema.tables.push(table_dict);
    }
    Ok(schema)
}

pub async fn review_schema(client: &dyn LlmClient, schema_json: &str) -> Result<String> {
    if schema_json.trim().is_empty() {
        bail!("内容为空，无法进行评审。");
    }
    let user = format!(
        "我将提供一份 MySQL 表结构定义，请你根据以上标准进行全面的 review，指出存在的问题并给出优化建议：\n\n{schema_json}"
    );
    let raw = client
        .chat(ChatRequest {
            messages: vec![ChatMessage::system(SYSTEM_PROMPT), ChatMessage::user(user)],
            ..Default::default()
        })
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(strip_markdown_fence(&raw))
}

pub async fn run_interactive() -> Result<()> {
    let host = get_user_input("请输入数据库地址", "localhost")?;
    let port = get_user_input_int("请输入数据库端口", 3306)? as u16;
    let user = get_user_input("请输入数据库用户名", "root")?;
    let password = read_password("请输入数据库密码: ")?;
    let database = require_non_empty("请输入数据库名称: ")?;
    let pattern = {
        let p = get_user_input(
            "请输入表名通配符 (支持LIKE语法，如 user%、log_%)，留空表示全部表",
            "",
        )?;
        if p.is_empty() {
            None
        } else {
            Some(p)
        }
    };
    run_with_args(
        MysqlConnectArgs {
            host,
            port,
            user,
            password,
            database,
            pattern,
        },
        false,
        false,
    )
    .await
}

fn read_password(prompt: &str) -> Result<String> {
    match rpassword::prompt_password(prompt) {
        Ok(p) if !p.is_empty() => Ok(p),
        Ok(_) => {
            println!("❌ 该项不能为空");
            read_password(prompt)
        }
        Err(_) => require_non_empty(prompt),
    }
}

pub async fn run_with_args(
    args: MysqlConnectArgs,
    dry_run: bool,
    skip_confirm: bool,
) -> Result<()> {
    let schema = tokio::task::spawn_blocking({
        let args = args.clone();
        move || fetch_mysql_schema(&args)
    })
    .await
    .context("join mysql task")??;

    if schema.tables.is_empty() {
        println!("❌ 未获取到任何表结构，程序退出。");
        return Ok(());
    }
    let text = serde_json::to_string_pretty(&schema).context("serialize schema")?;
    println!("即将进行 Review 的表结构如下：\n");
    println!("{text}");

    if dry_run {
        println!("\n（dry-run：未调用 LLM）");
        return Ok(());
    }
    if !skip_confirm && !confirm_action("是否确认发送 Review 请求？(y/n): ")? {
        println!("用户取消操作，退出程序。");
        return Ok(());
    }
    let client = factory_from_env().context("LLM factory")?;
    let result = review_schema(client.as_ref(), &text).await?;
    println!("Review 结果:\n{result}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_names() {
        assert!(is_safe_table_name("user_log"));
        assert!(is_safe_table_name("t$1"));
        assert!(!is_safe_table_name("user;drop"));
        assert!(!is_safe_table_name("a b"));
    }

    #[test]
    fn schema_json_shape() {
        let s = SchemaDump {
            tables: vec![TableInfo {
                name: "users".into(),
                columns: vec![ColumnInfo {
                    name: "id".into(),
                    col_type: "bigint".into(),
                    primary_key: true,
                    foreign_key: None,
                    comment: "".into(),
                }],
            }],
        };
        let j = serde_json::to_string_pretty(&s).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["tables"][0]["name"], "users");
        assert_eq!(v["tables"][0]["columns"][0]["primary_key"], true);
    }
}
