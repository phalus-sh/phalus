import Database from 'better-sqlite3';
import path from 'node:path';
import { CREATE_TABLES_SQL, MIGRATIONS_V2, SCHEMA_VERSION } from './schema.js';

let _db: Database.Database | null = null;

export function getDb(): Database.Database {
  if (!_db) {
    throw new Error('Database not initialized. Call initDb() first.');
  }
  return _db;
}

export function initDb(dbPath?: string): Database.Database {
  const resolvedPath = dbPath ?? process.env['PHALUS_DB_PATH'] ?? path.join(process.cwd(), 'phalus.db');
  const db = new Database(resolvedPath);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  db.exec(CREATE_TABLES_SQL);
  // Run v2 migrations for existing databases (swallow duplicate-column errors)
  for (const sql of MIGRATIONS_V2) {
    try {
      db.exec(sql);
    } catch {
      // Column already exists — safe to ignore
    }
  }
  // Record migration version
  db.prepare(
    `INSERT OR IGNORE INTO schema_migrations (version) VALUES (?)`
  ).run(SCHEMA_VERSION);
  _db = db;
  return db;
}

export function closeDb(): void {
  if (_db) {
    _db.close();
    _db = null;
  }
}
