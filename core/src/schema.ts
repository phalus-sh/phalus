// Database schema definitions for PHALUS
// Supports both SQLite (default) and Postgres (enterprise) via driver abstraction

export const SCHEMA_VERSION = 2;

export const CREATE_TABLES_SQL = `
  CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
  );

  CREATE TABLE IF NOT EXISTS packages (
    id TEXT PRIMARY KEY,
    ecosystem TEXT NOT NULL,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    license_expression TEXT,
    license_source TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(ecosystem, name, version)
  );

  CREATE TABLE IF NOT EXISTS policies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    rules TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
  );

  CREATE TABLE IF NOT EXISTS scan_runs (
    id TEXT PRIMARY KEY,
    project_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    started_at TEXT,
    finished_at TEXT,
    error TEXT,
    policy_id TEXT REFERENCES policies(id),
    policy_verdict TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  );

  CREATE TABLE IF NOT EXISTS scan_results (
    id TEXT PRIMARY KEY,
    scan_run_id TEXT NOT NULL REFERENCES scan_runs(id),
    package_id TEXT NOT NULL REFERENCES packages(id),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(scan_run_id, package_id)
  );

  CREATE TABLE IF NOT EXISTS alerts (
    id TEXT PRIMARY KEY,
    package_id TEXT REFERENCES packages(id),
    scan_run_id TEXT REFERENCES scan_runs(id),
    kind TEXT NOT NULL,
    severity TEXT NOT NULL DEFAULT 'medium',
    message TEXT NOT NULL,
    resolved_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
  );

  CREATE INDEX IF NOT EXISTS idx_packages_ecosystem_name ON packages(ecosystem, name);
  CREATE INDEX IF NOT EXISTS idx_scan_results_run ON scan_results(scan_run_id);
  CREATE INDEX IF NOT EXISTS idx_alerts_package ON alerts(package_id);
`;

/**
 * Migrations for existing databases (schema v1 → v2).
 * Each statement is tried individually; duplicate-column errors are swallowed.
 */
export const MIGRATIONS_V2 = [
  `ALTER TABLE scan_runs ADD COLUMN policy_id TEXT REFERENCES policies(id)`,
  `ALTER TABLE scan_runs ADD COLUMN policy_verdict TEXT`,
];
