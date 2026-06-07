use rusqlite::Connection;
use std::path::Path;

/// Open a connection with sane pragmas and ensure the schema exists.
pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // Wait (rather than instantly erroring with SQLITE_BUSY) if another
    // connection holds the write lock — e.g. a search hitting the DB while an
    // index run is mid-write.
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    init_schema(&conn)?;
    Ok(conn)
}

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS folders(
            id        INTEGER PRIMARY KEY,
            path      TEXT UNIQUE NOT NULL,
            added_at  INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS files(
            id         INTEGER PRIMARY KEY,
            folder_id  INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
            path       TEXT UNIQUE NOT NULL,
            mtime      INTEGER NOT NULL,
            hash       TEXT NOT NULL,
            size       INTEGER NOT NULL,
            indexed_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS chunks(
            id         INTEGER PRIMARY KEY,
            file_id    INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            ord        INTEGER NOT NULL,
            text       TEXT NOT NULL,
            heading    TEXT,
            char_start INTEGER
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_id);

        -- Standalone FTS5 table (rowid == chunks.id). trigram tokenizer is CJK-friendly.
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
            text,
            tokenize = 'trigram'
        );

        CREATE TABLE IF NOT EXISTS embeddings(
            chunk_id INTEGER PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
            vector   BLOB NOT NULL
        );
        "#,
    )
}
