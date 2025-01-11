use chrono::{DateTime, Local, Utc};
use sqlx;
use std::fmt;

#[derive(Debug, sqlx::FromRow)]
pub struct Entry {
    pub id: i64,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub pinned: bool,
}

impl fmt::Display for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let local_created_at = self.created_at.with_timezone(&Local);
        writeln!(f, "{}", local_created_at.format("%A, %B %-d, %Y %-I:%M %p"))?;
        writeln!(f, "-------------------------------------------")?;
        writeln!(f, "{}", self.content)?;
        writeln!(f, "-------------------------------------------")?;

        if let Some(date) = self.updated_at {
            let local_updated_at = date.with_timezone(&Local);
            writeln!(
                f,
                "Updated at: {}",
                local_updated_at.format("%A, %B %-d, %Y %-I:%M %p")
            )?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_without_updated_at() {
        let entry = Entry {
            id: 1,
            content: "Hello test".to_string(),
            created_at: DateTime::parse_from_str(
                "2024-01-01 12:30:00 +0000",
                "%Y-%m-%d %H:%M:%S %z",
            )
            .unwrap()
            .with_timezone(&Utc),
            updated_at: None,
            pinned: false,
        };

        let actual_output = entry.to_string();
        let expected_output = "Monday, January 1, 2024 2:30 PM\n\
                          -------------------------------------------\n\
                          Hello test\n\
                          -------------------------------------------\n";

        assert_eq!(actual_output, expected_output);
    }

    #[test]
    fn test_display_with_updated_at() {
        let entry = Entry {
            id: 1,
            content: "Hello test".to_string(),
            created_at: DateTime::parse_from_str(
                "2024-01-01 12:30:00 +0000",
                "%Y-%m-%d %H:%M:%S %z",
            )
            .unwrap()
            .with_timezone(&Utc),
            updated_at: Some(
                DateTime::parse_from_str("2025-02-02 12:30:00 +0000", "%Y-%m-%d %H:%M:%S %z")
                    .unwrap()
                    .with_timezone(&Utc),
            ),
            pinned: false,
        };

        let actual_output = entry.to_string();
        let expected_output = "Monday, January 1, 2024 2:30 PM\n\
                          -------------------------------------------\n\
                          Hello test\n\
                          -------------------------------------------\n\
                          Updated at: Sunday, February 2, 2025 2:30 PM\n";

        assert_eq!(actual_output, expected_output);
    }
}
