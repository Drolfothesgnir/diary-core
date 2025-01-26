#[cfg(test)]
mod tests {
    use super::super::*; // This accesses everything from mod.rs
    use crate::models::Entry;
    use anyhow::Result;
    use sqlx::sqlite::SqlitePool;

    async fn create_test_pool() -> Result<SqlitePool> {
        let db_url = "sqlite::memory:";
        let pool = SQLiteDiaryDB::create_schema(&db_url).await?;
        Ok(pool)
    }

    async fn create_sample_entries(db: &SQLiteDiaryDB) -> Result<Vec<Entry>> {
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
    async fn test_create_entry() {
        let pool = create_test_pool()
            .await
            .expect("Failed to create test pool");
        let db = SQLiteDiaryDB { pool };

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
    }

    #[tokio::test]
    async fn test_read_entries() {
        let pool = create_test_pool()
            .await
            .expect("Failed to create test pool");
        let db = SQLiteDiaryDB { pool };

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
    }

    #[tokio::test]
    async fn test_read_entry() {
        let pool = create_test_pool()
            .await
            .expect("Failed to create test pool");
        let db = SQLiteDiaryDB { pool };

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
    }

    #[tokio::test]
    async fn test_update_entry() {
        let pool = create_test_pool()
            .await
            .expect("Failed to create test pool");
        let db = SQLiteDiaryDB { pool };

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
    }

    #[tokio::test]
    async fn test_delete_entry() {
        let pool = create_test_pool()
            .await
            .expect("Failed to create test pool");
        let db = SQLiteDiaryDB { pool };

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
    }

    #[tokio::test]
    async fn test_pagination_default_values() -> Result<()> {
        let pool = create_test_pool().await?;
        let db = SQLiteDiaryDB { pool };

        // Create 15 test entries
        for i in 0..15 {
            let pinned = i < 3; // First 3 entries will be pinned
            let content = if i == 4 || i == 7 {
                format!("Entry {} search TEST", i) // Note mixed case
            } else {
                format!("Entry {}", i)
            };
            db.create_entry(content, pinned).await?;
        }

        // Test with default values (page 1, per_page 10)
        let pagination = db
            .read_entries_with_pagination(None, None, None, None, None)
            .await?;

        assert_eq!(pagination.total, 15);
        assert_eq!(pagination.entries.len(), 10); // Default per_page
        assert_eq!(pagination.page, 1);
        assert_eq!(pagination.per_page, 10);
        assert_eq!(pagination.total_pages, 2);
        assert!(pagination.has_next);

        db.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_pagination_custom_page_size() -> Result<()> {
        let pool = create_test_pool().await?;
        let db = SQLiteDiaryDB { pool };

        // Create test entries
        for i in 0..15 {
            db.create_entry(format!("Entry {}", i), false).await?;
        }

        // Test with 5 items per page
        let pagination = db
            .read_entries_with_pagination(Some(1), Some(5), None, None, None)
            .await?;

        assert_eq!(pagination.total, 15);
        assert_eq!(pagination.entries.len(), 5);
        assert_eq!(pagination.page, 1);
        assert_eq!(pagination.per_page, 5);
        assert_eq!(pagination.total_pages, 3);
        assert!(pagination.has_next);

        db.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_pagination_with_filters() -> Result<()> {
        let pool = create_test_pool().await?;
        let db = SQLiteDiaryDB { pool };

        // Create test entries with varying pinned status and content
        db.create_entry("First pinned".to_string(), true).await?;
        db.create_entry("Regular entry".to_string(), false).await?;
        db.create_entry("Second pinned SEARCH test".to_string(), true)
            .await?;
        db.create_entry("Another search TEST".to_string(), false)
            .await?;
        db.create_entry("Third pinned".to_string(), true).await?;

        // Test with pinned filter
        let pagination = db
            .read_entries_with_pagination(None, None, None, Some(true), None)
            .await?;

        assert_eq!(pagination.total, 3); // Only pinned entries
        assert_eq!(pagination.entries.len(), 3);
        assert_eq!(pagination.total_pages, 1);
        assert!(!pagination.has_next);

        // Test case-insensitive search - lowercase query
        let pagination = db
            .read_entries_with_pagination(None, None, None, None, Some("search test".to_string()))
            .await?;

        assert_eq!(pagination.total, 2); // Should match both "SEARCH test" and "search TEST"
        assert_eq!(pagination.entries.len(), 2);
        assert_eq!(pagination.total_pages, 1);
        assert!(!pagination.has_next);

        // Test case-insensitive search - uppercase query
        let pagination = db
            .read_entries_with_pagination(None, None, None, None, Some("SEARCH TEST".to_string()))
            .await?;

        assert_eq!(pagination.total, 2); // Should match the same entries
        assert_eq!(pagination.entries.len(), 2);
        assert_eq!(pagination.total_pages, 1);
        assert!(!pagination.has_next);

        // Test with combined filters (pinned + case-insensitive search)
        let pagination = db
            .read_entries_with_pagination(
                None,
                None,
                None,
                Some(true),
                Some("SEARCH test".to_string()),
            )
            .await?;

        assert_eq!(pagination.total, 1); // Only pinned entries with "search test"
        assert_eq!(pagination.entries.len(), 1);
        assert_eq!(pagination.total_pages, 1);
        assert!(!pagination.has_next);

        db.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_pagination_partial_substring() -> Result<()> {
        let pool = create_test_pool().await?;
        let db = SQLiteDiaryDB { pool };

        // Create entries with various text cases
        db.create_entry("Testing SUBSTRING match".to_string(), false)
            .await?;
        db.create_entry("TESTING another Match".to_string(), false)
            .await?;
        db.create_entry("Just a test".to_string(), false).await?;

        // Test partial word match with different cases
        let pagination = db
            .read_entries_with_pagination(None, None, None, None, Some("TEST".to_string()))
            .await?;

        assert_eq!(pagination.total, 3); // Should match all three entries
        assert_eq!(pagination.entries.len(), 3);

        // Test with mixed case substring
        let pagination = db
            .read_entries_with_pagination(None, None, None, None, Some("TeStInG".to_string()))
            .await?;

        assert_eq!(pagination.total, 2); // Should match first two entries
        assert_eq!(pagination.entries.len(), 2);

        db.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_pagination_invalid_params() -> Result<()> {
        let pool = create_test_pool().await?;
        let db = SQLiteDiaryDB { pool };

        // Create some test entries
        for i in 0..5 {
            db.create_entry(format!("Entry {}", i), false).await?;
        }

        // Test invalid page number
        let result = db
            .read_entries_with_pagination(Some(0), None, None, None, None)
            .await;
        assert!(result.is_err());

        // Test invalid per_page
        let result = db
            .read_entries_with_pagination(None, Some(0), None, None, None)
            .await;
        assert!(result.is_err());

        // Test page beyond available data
        let pagination = db
            .read_entries_with_pagination(Some(3), Some(2), None, None, None)
            .await?;

        assert_eq!(pagination.total, 5);
        assert_eq!(pagination.entries.len(), 1); // Last page with remaining entry
        assert_eq!(pagination.page, 3);
        assert_eq!(pagination.total_pages, 3);
        assert!(!pagination.has_next);

        db.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_pagination_sorting() -> Result<()> {
        let pool = create_test_pool().await?;
        let db = SQLiteDiaryDB { pool };

        // Create test entries
        for i in 0..5 {
            db.create_entry(format!("Entry {}", i), false).await?;
        }

        // Test ascending sort
        let pagination = db
            .read_entries_with_pagination(None, None, Some(SortOrder::ASC), None, None)
            .await?;

        assert_eq!(pagination.total, 5);
        let first_id = pagination.entries[0].id;
        let second_id = pagination.entries[1].id;
        assert!(
            first_id < second_id,
            "Entries should be sorted in ascending order"
        );

        // Test descending sort
        let pagination = db
            .read_entries_with_pagination(None, None, Some(SortOrder::DESC), None, None)
            .await?;

        assert_eq!(pagination.total, 5);
        let first_id = pagination.entries[0].id;
        let second_id = pagination.entries[1].id;
        assert!(
            first_id > second_id,
            "Entries should be sorted in descending order"
        );

        db.close().await;
        Ok(())
    }

    #[tokio::test]
    async fn test_dump_entries() -> Result<()> {
        let pool = create_test_pool()
            .await
            .expect("Failed to create test pool");
        let db = SQLiteDiaryDB { pool };

        // Create sample entries first
        let entries = create_sample_entries(&db)
            .await
            .expect("Failed to create sample entries");

        // Create a temporary file for the dump
        let temp_dir = tempfile::tempdir()?;
        let dump_path = temp_dir.path().join("test_dump.txt");

        // Dump entries to the temporary file
        db.dump_entries(Some(&dump_path)).await?;

        // Read the dumped content
        let dumped_content = std::fs::read_to_string(&dump_path)?;

        // Verify content - the dump should match the string representation of our entries
        let expected_content = entries
            .into_iter()
            .map(|entry| entry.to_string())
            .collect::<Vec<String>>()
            .join("\n\n");

        assert_eq!(dumped_content, expected_content);

        db.close().await;

        Ok(())
    }
}
