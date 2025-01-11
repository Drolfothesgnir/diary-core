#[cfg(test)]
mod tests {
    use crate::db::PostgresDiaryDB;
    use crate::db::SortOrder;
    use crate::models::Entry;
    use anyhow::Context;
    use anyhow::Result;
    use sqlx::postgres::PgPoolOptions;
    use uuid::Uuid;

    const POSTGRES_URL: &str = "postgres://postgres:password@localhost:5432/postgres";

    struct TestDB {
        db_name: String,
    }

    impl TestDB {
        async fn new() -> Result<(Self, PostgresDiaryDB)> {
            let db_name = format!("test_diary_{}", Uuid::new_v4().simple());

            let test_db_url = format!("postgres://postgres:password@localhost:5432/{}", db_name);

            let postgres_pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(POSTGRES_URL)
                .await
                .context("Failed to connect to Postgres db")?;

            sqlx::query(&format!("CREATE DATABASE \"{}\";", db_name))
                .execute(&postgres_pool)
                .await
                .context("Failed to create test db")?;

            postgres_pool.close().await;

            let db = PostgresDiaryDB::new(&test_db_url).await?;

            Ok((Self { db_name }, db))
        }

        async fn cleanup(&self) -> Result<()> {
            let pg_pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(POSTGRES_URL)
                .await
                .context("Failed to connect to postgres db")?;

            sqlx::query(&format!("DROP DATABASE IF EXISTS \"{}\";", self.db_name))
                .execute(&pg_pool)
                .await
                .context("Failed to delete test db")?;

            pg_pool.close().await;

            Ok(())
        }
    }

    async fn create_sample_entries(db: &PostgresDiaryDB) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();

        entries.push(db.create_entry("First entry".to_string(), true).await?);
        entries.push(db.create_entry("Second entry".to_string(), false).await?);
        entries.push(
            db.create_entry("Third pinned entry".to_string(), true)
                .await?,
        );

        Ok(entries)
    }

    #[tokio::test]
    async fn test_create_entry() -> Result<()> {
        let (test_db, db) = TestDB::new().await?;

        let content = "Test entry content";
        let pinned = true;

        let created_entry = db
            .create_entry(content.to_string(), pinned)
            .await
            .expect("Failed to create entry");

        assert_eq!(created_entry.id, 1);
        assert!(db.check_if_entry_exists(1).await.unwrap());
        assert_eq!(created_entry.content, content);
        assert_eq!(created_entry.pinned, pinned);

        db.close().await;
        test_db.cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_read_entries() -> Result<()> {
        let (test_db, db) = TestDB::new().await?;

        let entries = create_sample_entries(&db)
            .await
            .expect("Failed to create sample entries");

        // Test default pagination (page 1, per_page 10)
        let results = db
            .read_entries(None, None, None, None, None)
            .await
            .expect("Failed to read entries");
        assert_eq!(results.len(), 3);

        // Test pagination
        let paginated = db
            .read_entries(Some(1), Some(2), None, None, None)
            .await
            .expect("Failed to read entries");
        assert_eq!(paginated.len(), 2);

        // Test pinned filter
        let pinned = db
            .read_entries(None, None, None, Some(true), None)
            .await
            .expect("Failed to read entries");
        assert_eq!(pinned.len(), 2);

        // Test substring search
        let search = db
            .read_entries(None, None, None, None, Some("Second".to_string()))
            .await
            .expect("Failed to read entries");
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].content, "Second entry");

        // Test sorting
        let asc_sorted = db
            .read_entries(None, None, Some(SortOrder::ASC), None, None)
            .await
            .expect("Failed to read entries");
        assert_eq!(asc_sorted[0].id, entries[0].id);

        db.close().await;
        test_db.cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_read_entry() -> Result<()> {
        let (test_db, db) = TestDB::new().await?;

        let entry = db
            .create_entry("Test entry".to_string(), false)
            .await
            .expect("Failed to create entry");

        // Test successful read
        let read_entry = db.read_entry(entry.id).await.expect("Failed to read entry");
        assert_eq!(read_entry.id, entry.id);
        assert_eq!(read_entry.content, entry.content);
        assert_eq!(read_entry.pinned, entry.pinned);

        // Test reading non-existent entry
        let non_existent = db.read_entry(999).await;
        assert!(non_existent.is_err());

        db.close().await;
        test_db.cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_update_entry() -> Result<()> {
        let (test_db, db) = TestDB::new().await?;

        let entry = db
            .create_entry("Original content".to_string(), false)
            .await
            .expect("Failed to create entry");

        // Test updating content only
        let updated_content = db
            .update_entry(entry.id, Some("Updated content".to_string()), None)
            .await
            .expect("Failed to update entry content");
        assert_eq!(updated_content.content, "Updated content");
        assert_eq!(updated_content.pinned, false);

        // Test updating pinned status only
        let updated_pinned = db
            .update_entry(entry.id, None, Some(true))
            .await
            .expect("Failed to update entry pinned status");
        assert_eq!(updated_pinned.content, "Updated content");
        assert_eq!(updated_pinned.pinned, true);

        // Test updating both fields
        let fully_updated = db
            .update_entry(entry.id, Some("Both updated".to_string()), Some(false))
            .await
            .expect("Failed to update entry completely");
        assert_eq!(fully_updated.content, "Both updated");
        assert_eq!(fully_updated.pinned, false);

        // Test updating non-existent entry
        let non_existent = db
            .update_entry(999, Some("Should fail".to_string()), None)
            .await;
        assert!(non_existent.is_err());

        // Test updating with no fields
        let no_fields = db.update_entry(entry.id, None, None).await;
        assert!(no_fields.is_err());

        db.close().await;
        test_db.cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_entry() -> Result<()> {
        let (test_db, db) = TestDB::new().await?;

        let entry = db
            .create_entry("To be deleted".to_string(), false)
            .await
            .expect("Failed to create entry");

        // Verify entry exists
        assert!(db.check_if_entry_exists(entry.id).await.unwrap());

        // Test successful deletion
        let delete_result = db.delete_entry(entry.id).await;
        assert!(delete_result.is_ok());

        // Verify entry no longer exists
        assert!(!db.check_if_entry_exists(entry.id).await.unwrap());

        // Test deleting non-existent entry
        let non_existent = db.delete_entry(999).await;
        assert!(non_existent.is_err());

        db.close().await;
        test_db.cleanup().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_entry_timestamps() -> Result<()> {
        let (test_db, db) = TestDB::new().await?;

        // Create an entry and immediately read it back
        let entry = db.create_entry("Test entry".to_string(), false).await?;
        let read_entry = db.read_entry(entry.id).await?;

        // Check that created_at is preserved correctly
        assert_eq!(entry.created_at, read_entry.created_at);
        assert!(read_entry.updated_at.is_none());

        // Update the entry and verify updated_at is set
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await; // Ensure timestamp will be different
        let updated = db
            .update_entry(entry.id, Some("Updated content".to_string()), None)
            .await?;

        // Verify updated_at is set and is after created_at
        assert!(updated.updated_at.is_some());
        assert!(updated.updated_at.unwrap() > updated.created_at);

        // Read back and verify timestamps are preserved
        let read_updated = db.read_entry(entry.id).await?;
        assert_eq!(updated.updated_at, read_updated.updated_at);

        db.close().await;
        test_db.cleanup().await?;
        Ok(())
    }
}
