use super::{Pagination, SortOrder};
use crate::models::Entry;
use anyhow::{Context, Result};
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool};
pub struct SQLiteDiaryDB {
    pub pool: SqlitePool,
}

impl SQLiteDiaryDB {
    pub async fn new(db_url: &str) -> Result<Self> {
        if !Sqlite::database_exists(&db_url).await? {
            Sqlite::create_database(&db_url).await?;
            let pool = Self::create_schema(&db_url).await?;
            println!("Database created successfully");
            return Ok(Self { pool });
        }

        let pool = SqlitePool::connect(db_url)
            .await
            .context("Failed to connect to the database")?;

        Ok(Self { pool })
    }

    pub async fn create_schema(db_url: &str) -> Result<SqlitePool> {
        let pool = SqlitePool::connect(db_url)
            .await
            .context("Failed to connect to the database")?;

        let qry = "
          CREATE TABLE IF NOT EXISTS entries (
              id         INTEGER PRIMARY KEY NOT NULL,
              content    TEXT NOT NULL,
              created_at DATETIME NOT NULL DEFAULT (datetime('now')),
              updated_at DATETIME DEFAULT (datetime('now')),
              pinned     BOOLEAN NOT NULL DEFAULT 0
          );

          CREATE TRIGGER IF NOT EXISTS update_entries_updated_at
          AFTER UPDATE ON entries
          FOR EACH ROW
          BEGIN
              UPDATE entries
              SET updated_at = datetime('now')
              WHERE id = OLD.id;
          END;
      ";

        sqlx::query(&qry)
            .execute(&pool)
            .await
            .context("Failed to create database schema")?;

        Ok(pool)
    }

    pub async fn create_entry(&self, content: String, pinned: bool) -> Result<Entry> {
        let qry = "INSERT INTO entries (content, pinned) VALUES($1, $2) RETURNING *;";
        let result = sqlx::query_as::<_, Entry>(qry)
            .bind(content)
            .bind(pinned)
            .fetch_one(&self.pool)
            .await
            .context("Failed to create an entry")?;
        println!("New entry created.");
        Ok(result)
    }

    fn build_base_query(
        selector: &str,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> (String, i32) {
        let mut query = format!("SELECT {} FROM entries", selector);
        let mut conditions = Vec::new();
        let mut param_count = 0;

        if pinned.is_some() {
            param_count += 1;
            conditions.push(format!("pinned = ${}", param_count));
        }

        if substring.is_some() {
            param_count += 1;
            conditions.push(format!("LOWER(content) LIKE LOWER(${})", param_count));
        }

        if !conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&conditions.join(" AND "));
        }

        (query, param_count)
    }

    fn build_search_query(
        selector: &str,
        page: i64,
        per_page: i64,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<String> {
        if page < 1 || per_page < 1 {
            return Err(anyhow::anyhow!("Page and per_page must be positive"));
        }

        let sort = sort.unwrap_or(SortOrder::DESC);
        let order = match sort {
            SortOrder::ASC => "ASC",
            SortOrder::DESC => "DESC",
        };

        let (mut query, param_count) = Self::build_base_query(selector, pinned, substring);

        // Only add ORDER BY, LIMIT, OFFSET for data query, not for COUNT
        if selector != "COUNT(*)" {
            query.push_str(&format!(
                " ORDER BY created_at {0}, id {0} LIMIT ${1} OFFSET ${2};",
                order,
                param_count + 1,
                param_count + 2
            ));
        }

        Ok(query)
    }

    pub async fn read_entries(
        &self,
        page: Option<i64>,
        per_page: Option<i64>,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<Vec<Entry>> {
        let page = page.unwrap_or(1);
        let per_page = per_page.unwrap_or(10);
        let offset = (page - 1) * per_page;
        let query = Self::build_search_query("*", page, per_page, sort, pinned, substring.clone())?;
        println!("{}", query);

        let mut query_builder = sqlx::query_as::<_, Entry>(&query);

        if let Some(is_pinned) = pinned {
            query_builder = query_builder.bind(is_pinned);
        }

        if let Some(substr) = substring {
            query_builder = query_builder.bind(format!("%{}%", substr));
        }

        query_builder
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .context("Failed to read entries")
    }

    pub async fn read_entries_with_pagination(
        &self,
        page: Option<i64>,
        per_page: Option<i64>,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<Pagination> {
        let page = page.unwrap_or(1);
        let per_page = per_page.unwrap_or(10);

        // First get total count
        let count_query = Self::build_base_query("COUNT(*)", pinned, substring.clone()).0;
        let mut count_builder = sqlx::query_scalar(&count_query);

        if let Some(is_pinned) = pinned {
            count_builder = count_builder.bind(is_pinned);
        }

        if let Some(substr) = substring.clone() {
            count_builder = count_builder.bind(format!("%{}%", substr));
        }

        let total: i64 = count_builder
            .fetch_one(&self.pool)
            .await
            .context("Failed to count number of entries")?;

        // Then get paginated entries
        let entries = self
            .read_entries(Some(page), Some(per_page), sort, pinned, substring)
            .await?;

        let has_next = (page * per_page) < total;
        let total_pages = (total + per_page - 1) / per_page;

        Ok(Pagination {
            entries,
            has_next,
            total,
            page,
            per_page,
            total_pages,
        })
    }

    pub async fn check_if_entry_exists(&self, id: i64) -> Result<bool> {
        let result = sqlx::query("SELECT 1 FROM entries WHERE id = $1;")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .context(format!("Failed to check if entry with id: {} exists", id))?;

        Ok(result.is_some())
    }

    pub async fn read_entry(&self, id: i64) -> Result<Entry> {
        let qry = "
          SELECT * FROM entries
          WHERE id = $1;
      ";

        sqlx::query_as::<_, Entry>(&qry)
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .context(format!("Failed to read entry with id: {}", id))
    }

    pub async fn update_entry(
        &self,
        id: i64,
        content: Option<String>,
        pinned: Option<bool>,
    ) -> Result<Entry> {
        let entry_exists = self.check_if_entry_exists(id).await?;

        if !entry_exists {
            return Err(anyhow::anyhow!("Entry with id: {} doesn't exist", id));
        }

        let mut query_parts = Vec::new();
        let mut param_count = 1;

        if content.is_some() {
            param_count += 1;
            query_parts.push(format!("content = ${}", param_count));
        }

        if pinned.is_some() {
            param_count += 1;
            query_parts.push(format!("pinned = ${}", param_count));
        }

        if content.is_none() && pinned.is_none() {
            return Err(anyhow::anyhow!(
                "At least one field must be provided for update"
            ));
        }

        let qry = format!(
            "
          UPDATE entries
          SET {}
          WHERE id = $1
          RETURNING *;
      ",
            query_parts.join(", ")
        );

        let mut query_builder = sqlx::query_as::<_, Entry>(&qry).bind(id);

        if let Some(new_content) = content {
            query_builder = query_builder.bind(new_content);
        }

        if let Some(new_pinned) = pinned {
            query_builder = query_builder.bind(new_pinned);
        }

        println!("Entry with id: {} updated.", id);

        query_builder
            .fetch_one(&self.pool)
            .await
            .context(format!("Failed to update an entry with id: {}", id))
    }

    pub async fn delete_entry(&self, id: i64) -> Result<()> {
        let entry_exists = self.check_if_entry_exists(id).await?;

        if !entry_exists {
            return Err(anyhow::anyhow!("Entry with id: {} doesn't exist", id));
        }

        let qry = "DELETE FROM entries WHERE id = $1";
        sqlx::query(&qry)
            .bind(id)
            .execute(&self.pool)
            .await
            .context(format!("Failed to delete entry with id: {}", id))?;
        println!("Entry with id: {} deleted.", id);

        Ok(())
    }

    pub async fn close(&self) {
        self.pool.close().await;
        println!("\nDatabase connection closed\n")
    }
}
