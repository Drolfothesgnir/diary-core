# Database Migration Guide

This guide explains how to adjust the database after loading it from a backup. These steps are necessary to ensure compatibility with the Rust diary application.

## Prerequisites

- PostgreSQL installed and running
- Access to the target database
- A backup of the source database loaded

## Migration Steps

1. First, drop the existing entries table if it exists:
```sql
DROP TABLE IF EXISTS entries;
```

2. Rename the records table to entries:
```sql
ALTER TABLE records RENAME TO entries;
```

3. Modify the id column to use BIGINT type and set up the sequence:
```sql
ALTER TABLE entries
ALTER COLUMN id SET DATA TYPE BIGINT,
ALTER COLUMN id SET DEFAULT nextval('records_id_seq');
```

4. Update timestamp columns to use TIMESTAMPTZ:
```sql
ALTER TABLE entries
ALTER COLUMN created_at SET DATA TYPE TIMESTAMPTZ USING created_at AT TIME ZONE 'UTC',
ALTER COLUMN updated_at SET DATA TYPE TIMESTAMPTZ USING updated_at AT TIME ZONE 'UTC';

-- Set default values
ALTER TABLE entries 
ALTER COLUMN created_at SET DEFAULT CURRENT_TIMESTAMP;

ALTER TABLE entries 
ALTER COLUMN pinned SET DEFAULT FALSE;
```

## Verification

After running these commands, you can verify the table structure with:
```sql
\d entries
```

The output should show:
- `id` column as BIGINT
- `created_at` and `updated_at` columns as TIMESTAMPTZ

## Notes

- These steps assume your backup was from a database where the table was named 'records'
- The sequence name 'records_id_seq' is preserved from the original table
- Timestamps are converted to UTC timezone during the migration
- Make sure to backup your data before running these commands

## Troubleshooting

If you encounter any issues:
1. Verify that the backup was loaded successfully
2. Check that the sequence exists before running the ALTER TABLE commands
3. Confirm that you have the necessary permissions on the database