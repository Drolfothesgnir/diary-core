use super::{Pagination, SortOrder};
use crate::models::Entry;
use anyhow::{Context, Result};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::str::FromStr;

pub struct PostgresDiaryDB {
    pub pool: PgPool,
}

impl PostgresDiaryDB {
    pub async fn new(db_url: &str) -> Result<Self> {
        // Parse the connection string to get database name
        let opts = PgConnectOptions::from_str(db_url).context("Invalid database URL")?;
        let db_name = opts
            .get_database()
            .ok_or_else(|| anyhow::anyhow!("Database name not specified in URL"))?;

        // Create a connection to postgres database to check if our db exists
        let postgres_url = db_url.replace(db_name, "postgres");
        let postgres_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&postgres_url)
            .await
            .context("Failed to connect to postgres database")?;

        // Check if database exists
        let row: Option<(bool,)> =
            sqlx::query_as("SELECT TRUE FROM pg_database WHERE datname = $1")
                .bind(db_name)
                .fetch_optional(&postgres_pool)
                .await
                .context("Failed to check if database exists")?;

        // Create database if it doesn't exist
        if row.is_none() {
            sqlx::query(&format!("CREATE DATABASE \"{}\";", db_name))
                .execute(&postgres_pool)
                .await
                .context("Failed to create database")?;
            println!("Database created successfully");
        }

        // Close connection to postgres database
        postgres_pool.close().await;

        // Connect to the target database
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(db_url)
            .await
            .context("Failed to connect to the database")?;

        // Create schema
        Self::create_schema(&pool).await?;
        println!("Database connected successfully");

        Ok(Self { pool })
    }

    async fn create_schema(pool: &PgPool) -> Result<()> {
        // Drop existing trigger and function first
        let drop_trigger = "DROP TRIGGER IF EXISTS update_entries_updated_at ON entries;";
        let drop_function = "DROP FUNCTION IF EXISTS update_updated_at_column();";

        // Create table with TIMESTAMPTZ
        let create_table = "
            CREATE TABLE IF NOT EXISTS entries (
                id         BIGSERIAL PRIMARY KEY,
                content    TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMPTZ,
                pinned     BOOLEAN NOT NULL DEFAULT FALSE
            );";

        // Create update function
        let create_function = "
            CREATE OR REPLACE FUNCTION update_updated_at_column()
            RETURNS TRIGGER AS $$
            BEGIN
                IF NEW.* IS DISTINCT FROM OLD.* THEN
                    NEW.updated_at = CURRENT_TIMESTAMP;
                END IF;
                RETURN NEW;
            END;
            $$ language 'plpgsql';";

        // Create trigger
        let create_trigger = "
            CREATE TRIGGER update_entries_updated_at
                BEFORE UPDATE ON entries
                FOR EACH ROW
                EXECUTE FUNCTION update_updated_at_column();";

        // Set timezone to UTC for the database connection
        sqlx::query("SET TIME ZONE 'UTC';")
            .execute(pool)
            .await
            .context("Failed to set timezone")?;

        // Execute each query in order
        sqlx::query(drop_trigger)
            .execute(pool)
            .await
            .context("Failed to drop old trigger")?;

        sqlx::query(drop_function)
            .execute(pool)
            .await
            .context("Failed to drop old function")?;

        sqlx::query(create_table)
            .execute(pool)
            .await
            .context("Failed to create table")?;

        sqlx::query(create_function)
            .execute(pool)
            .await
            .context("Failed to create update function")?;

        sqlx::query(create_trigger)
            .execute(pool)
            .await
            .context("Failed to create trigger")?;

        Ok(())
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
            conditions.push(format!("content ILIKE ${}", param_count));
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
        let qry = "SELECT * FROM entries WHERE id = $1;";

        sqlx::query_as::<_, Entry>(qry)
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
            "UPDATE entries SET {} WHERE id = $1 RETURNING *;",
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
        sqlx::query(qry)
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
