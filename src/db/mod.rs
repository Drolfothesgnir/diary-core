pub mod postgres;
pub mod sqlite;
pub mod tests_postgres;
pub mod tests_sqlite;

use crate::models::Entry;
use anyhow::Result;
use async_trait::async_trait;
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, ValueEnum, Serialize, Deserialize)]
pub enum SortOrder {
    ASC,
    DESC,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Pagination {
    pub entries: Vec<Entry>,
    pub has_next: bool,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
    pub total_pages: i64,
}

#[async_trait]
pub trait DB {
    // Changed &str to String
    async fn create_entry(&self, content: String, pinned: bool) -> Result<Entry>;

    async fn read_entries(
        &self,
        page: Option<i64>,
        per_page: Option<i64>,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<Vec<Entry>>;

    async fn read_entries_with_pagination(
        &self,
        page: Option<i64>,
        per_page: Option<i64>,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<Pagination>;

    // Rest remains the same
    async fn check_if_entry_exists(&self, id: i64) -> Result<bool>;
    async fn read_entry(&self, id: i64) -> Result<Entry>;
    async fn update_entry(
        &self,
        id: i64,
        content: Option<String>,
        pinned: Option<bool>,
    ) -> Result<Entry>;
    async fn delete_entry(&self, id: i64) -> Result<()>;
    async fn dump_entries(&self, path: Option<&PathBuf>) -> Result<()>;
    async fn close(&self);
}

// Re-export the types you want available at the db module level
pub use self::postgres::PostgresDiaryDB;
pub use self::sqlite::SQLiteDiaryDB;

#[async_trait]
impl DB for SQLiteDiaryDB {
    // All these methods already exist in your SQLiteDiaryDB impl,
    // we're just adding them to the trait implementation
    async fn create_entry(&self, content: String, pinned: bool) -> Result<Entry> {
        self.create_entry(content, pinned).await
    }

    async fn read_entries(
        &self,
        page: Option<i64>,
        per_page: Option<i64>,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<Vec<Entry>> {
        self.read_entries(page, per_page, sort, pinned, substring)
            .await
    }

    async fn read_entries_with_pagination(
        &self,
        page: Option<i64>,
        per_page: Option<i64>,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<Pagination> {
        self.read_entries_with_pagination(page, per_page, sort, pinned, substring)
            .await
    }

    async fn check_if_entry_exists(&self, id: i64) -> Result<bool> {
        self.check_if_entry_exists(id).await
    }

    async fn read_entry(&self, id: i64) -> Result<Entry> {
        self.read_entry(id).await
    }

    async fn update_entry(
        &self,
        id: i64,
        content: Option<String>,
        pinned: Option<bool>,
    ) -> Result<Entry> {
        self.update_entry(id, content, pinned).await
    }

    async fn delete_entry(&self, id: i64) -> Result<()> {
        self.delete_entry(id).await
    }

    async fn dump_entries(&self, path: Option<&PathBuf>) -> Result<()> {
        self.dump_entries(path).await
    }

    async fn close(&self) {
        self.close().await
    }
}

#[async_trait]
impl DB for PostgresDiaryDB {
    // All these methods already exist in your SQLiteDiaryDB impl,
    // we're just adding them to the trait implementation
    async fn create_entry(&self, content: String, pinned: bool) -> Result<Entry> {
        self.create_entry(content, pinned).await
    }

    async fn read_entries(
        &self,
        page: Option<i64>,
        per_page: Option<i64>,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<Vec<Entry>> {
        self.read_entries(page, per_page, sort, pinned, substring)
            .await
    }

    async fn read_entries_with_pagination(
        &self,
        page: Option<i64>,
        per_page: Option<i64>,
        sort: Option<SortOrder>,
        pinned: Option<bool>,
        substring: Option<String>,
    ) -> Result<Pagination> {
        self.read_entries_with_pagination(page, per_page, sort, pinned, substring)
            .await
    }

    async fn check_if_entry_exists(&self, id: i64) -> Result<bool> {
        self.check_if_entry_exists(id).await
    }

    async fn read_entry(&self, id: i64) -> Result<Entry> {
        self.read_entry(id).await
    }

    async fn update_entry(
        &self,
        id: i64,
        content: Option<String>,
        pinned: Option<bool>,
    ) -> Result<Entry> {
        self.update_entry(id, content, pinned).await
    }

    async fn delete_entry(&self, id: i64) -> Result<()> {
        self.delete_entry(id).await
    }

    async fn dump_entries(&self, path: Option<&PathBuf>) -> Result<()> {
        self.dump_entries(path).await
    }

    async fn close(&self) {
        self.close().await
    }
}

pub struct DiaryDB {
    pub db: Box<dyn DB + Send + Sync>,
}

impl DiaryDB {
    pub async fn new(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let db = if url.starts_with("sqlite:") {
            let db = SQLiteDiaryDB::new(url).await?;
            Box::new(db) as Box<dyn DB + Send + Sync>
        } else if url.starts_with("postgres:") {
            let db = PostgresDiaryDB::new(url).await?;
            Box::new(db) as Box<dyn DB + Send + Sync>
        } else {
            return Err("Unsupported database URL".into());
        };

        Ok(Self { db })
    }
}
