use sqlx::{Sqlite, SqlitePool, Transaction, migrate::MigrateDatabase};
use std::fs;

use crate::{Entry}; // from src/lib.rs

type AnyError = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, AnyError>;

/// Returns true if the string contains any kanji (CJK Unified Ideographs).
pub fn contains_kanji(s: &str) -> bool {
    s.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c))
}

/// The URL to the SQLite database file.
pub const DB_URL: &str = "sqlite:data/jmdict_e.db"; 

/// Represents the dictionary database connection and operations.
pub struct DictDb {
    pool: SqlitePool,
}



impl DictDb {
    /// Connects to the JMdict SQLite database and initializes the schema if needed.
    /// 
    /// # Arguments
    /// * `db_url` - The database URL, e.g. "sqlite://jmdict.db"
    pub async fn connect(db_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(db_url).await?;
        let db = Self { pool };
        db.init_schema().await?;
        Ok(db)
    }

    /// Resets the database by dropping it and creating a new one.
    /// 
    /// # Arguments
    /// * `db_url` - The database URL, e.g. "sqlite://jmdict.db"
    pub async fn reset_database(db_url: &str) -> Result<()> {
        // Remove the database file if it exists
        if Sqlite::database_exists(db_url).await? {
            Sqlite::drop_database(db_url).await?;
        }

        // Remove the -shm and -wal files if they exist
        let shm_file = format!("{}-shm", db_url);
        let wal_file = format!("{}-wal", db_url);

        if fs::metadata(&shm_file).is_ok() {
            fs::remove_file(&shm_file)?;
        }

        if fs::metadata(&wal_file).is_ok() {
            fs::remove_file(&wal_file)?;
        }

        // Create a new database
        Sqlite::create_database(db_url).await?;
        Ok(())
    }

    
    /// Initializes the database schema.
    async fn init_schema(&self) -> Result<()> {
        // JSON arrays for all list fields
        let sql = r#"
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS entries (
          ent_seq  INTEGER PRIMARY KEY,
          rebs     TEXT NOT NULL, -- JSON array of readings
          kebs     TEXT NULL      -- JSON array of kanji or NULL
        );

        CREATE TABLE IF NOT EXISTS senses (
          id           INTEGER PRIMARY KEY AUTOINCREMENT,
          ent_seq      INTEGER NOT NULL REFERENCES entries(ent_seq) ON DELETE CASCADE,
          sense_order  INTEGER NOT NULL,
          pos          TEXT,  -- JSON array of strings
          xref         TEXT,  -- JSON array of strings
          gloss        TEXT   -- JSON array of strings
        );

        CREATE INDEX IF NOT EXISTS idx_senses_entry ON senses(ent_seq);
        "#;

        sqlx::query(sql).execute(&self.pool).await?;
        Ok(())
    }

    // Batch insert all entries in one transaction
    pub async fn insert_entries(&self, entries: &[Entry]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for e in entries {
            Self::upsert_entry_tx(&mut tx, e).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Insert/replace a single Entry (and its senses)
    async fn upsert_entry_tx(tx: &mut Transaction<'_, Sqlite>, e: &Entry) -> Result<()> {
        let ent_seq: i64 = e.ent_seq.parse()?;

        let rebs: Vec<&str> = e.r_ele.iter().map(|r| r.reb.as_str()).collect();
        let rebs_json = serde_json::to_string(&rebs)?;

        let kebs_json: Option<String> = e
            .k_ele
            .as_ref()
            .map(|ks| {
                let v: Vec<&str> = ks.iter().map(|k| k.keb.as_str()).collect();
                serde_json::to_string(&v).map_err::<AnyError, _>(|err| Box::new(err))
            })
            .transpose()?;

        // Upsert entry
        sqlx::query(
            r#"
            INSERT INTO entries (ent_seq, rebs, kebs)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(ent_seq) DO UPDATE SET
              rebs = excluded.rebs,
              kebs = excluded.kebs
            "#,
        )
        .bind(ent_seq)
        .bind(rebs_json)
        .bind(kebs_json)
        .execute(&mut **tx)
        .await?;

        // Replace senses for this entry
        sqlx::query("DELETE FROM senses WHERE ent_seq = ?1")
            .bind(ent_seq)
            .execute(&mut **tx)
            .await?;

        for (i, s) in e.sense.iter().enumerate() {
            let pos_json = serde_json::to_string(&s.pos)?;
            let xref_json = serde_json::to_string(&s.xref)?;
            let gloss_json = serde_json::to_string(&s.gloss)?;

            sqlx::query(
                r#"
                INSERT INTO senses (ent_seq, sense_order, pos, xref, gloss)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
            )
            .bind(ent_seq)
            .bind(i as i64)
            .bind(pos_json)
            .bind(xref_json)
            .bind(gloss_json)
            .execute(&mut **tx)
            .await?;
        }

        Ok(())
    }

   
    /// Searches for entries by reading (reb) and returns all matching entries with their senses.
    /// 
    /// # Arguments
    /// * `reading` - The reading string to search for.
    /// 
    /// # Returns
    /// A vector of EntryParsed.
    pub async fn search_entries_with_senses_by_reading(&self, reading: &str) -> Result<Vec<EntryParsed>> {
        // Find all matching entries
        let entry_rows = sqlx::query_as::<_, EntryRow>(
            r#"
            SELECT * FROM entries
            WHERE EXISTS (
                SELECT 1 FROM json_each(entries.rebs) je WHERE je.value = ?1
            )
            "#,
        )
        .bind(reading)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();

        for entry in entry_rows {
            // Parse rebs and kebs
            let rebs: Vec<String> = serde_json::from_str(&entry.rebs)?;
            let kebs: Option<Vec<String>> = match &entry.kebs {
                Some(s) => Some(serde_json::from_str(s)?),
                None => None,
            };

            // Get and parse all senses for this entry
            let sense_rows = sqlx::query_as::<_, SenseRow>(
                "SELECT * FROM senses WHERE ent_seq = ? ORDER BY sense_order"
            )
            .bind(entry.ent_seq)
            .fetch_all(&self.pool)
            .await?;

            let senses = sense_rows
                .into_iter()
                .map(|sense| {
                    Ok(SenseParsed {
                        sense_order: sense.sense_order,
                        pos: serde_json::from_str(&sense.pos)?,
                        xref: serde_json::from_str(&sense.xref)?,
                        gloss: serde_json::from_str(&sense.gloss)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            results.push(EntryParsed {
                ent_seq: entry.ent_seq,
                rebs,
                kebs,
                senses,
            });
        }

        Ok(results)
    }

    /// Searches for entries with their senses by kanji.
    /// 
    /// # Arguments
    /// * `kanji` - The kanji string to search for.
    /// 
    /// # Returns
    /// A vector of EntryParsed structs.
    pub async fn search_entries_with_senses_by_kanji(&self, kanji: &str) -> Result<Vec<EntryParsed>> {
        // Find all matching entries by kanji
        let entry_rows = sqlx::query_as::<_, EntryRow>(
            r#"
            SELECT * FROM entries
            WHERE kebs IS NOT NULL
              AND EXISTS (
                SELECT 1 FROM json_each(entries.kebs) je WHERE je.value = ?1
              )
            "#,
        )
        .bind(kanji)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();

        for entry in entry_rows {
            // Parse rebs and kebs
            let rebs: Vec<String> = serde_json::from_str(&entry.rebs)?;
            let kebs: Option<Vec<String>> = match &entry.kebs {
                Some(s) => Some(serde_json::from_str(s)?),
                None => None,
            };

            // Get and parse all senses for this entry
            let sense_rows = sqlx::query_as::<_, SenseRow>(
                "SELECT * FROM senses WHERE ent_seq = ? ORDER BY sense_order"
            )
            .bind(entry.ent_seq)
            .fetch_all(&self.pool)
            .await?;

            let senses = sense_rows
                .into_iter()
                .map(|sense| {
                    Ok(SenseParsed {
                        sense_order: sense.sense_order,
                        pos: serde_json::from_str(&sense.pos)?,
                        xref: serde_json::from_str(&sense.xref)?,
                        gloss: serde_json::from_str(&sense.gloss)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            results.push(EntryParsed {
                ent_seq: entry.ent_seq,
                rebs,
                kebs,
                senses,
            });
        }

        Ok(results)
    }
}

use sqlx::FromRow;

#[derive(Debug, FromRow)]
/// Represents a row in the entries table.
pub struct EntryRow {
    pub ent_seq: i64,
    pub rebs: String,         // JSON array as string
    pub kebs: Option<String>, // JSON array as string or None
}

#[derive(Debug, FromRow)]
/// Represents a row in the senses table.
pub struct SenseRow {
    pub id: i64,
    pub ent_seq: i64,
    pub sense_order: i64,
    pub pos: String,    // JSON array as string
    pub xref: String,   // JSON array as string
    pub gloss: String,  // JSON array as string
}


#[derive(Debug)]
/// Represents a parsed dictionary entry.
pub struct EntryParsed {
    /// The entry sequence number. this directly from JMdict.
    pub ent_seq: i64,
    /// The Japanese readings for this entry.
    pub rebs: Vec<String>,
    /// The kanji for this entry, if available.
    pub kebs: Option<Vec<String>>,
    /// The senses for this entry. It contains all the meanings and usages.
    pub senses: Vec<SenseParsed>,
}

#[derive(Debug)]
/// Represents a parsed sense entry.
pub struct SenseParsed {
    pub sense_order: i64,
    /// The part of speech for this sense.
    pub pos: Vec<String>,
    /// Cross-references for this sense.
    pub xref: Vec<String>,
    /// Glosses (meanings) for this sense.
    pub gloss: Vec<String>,
}

