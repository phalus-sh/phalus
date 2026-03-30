import fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import type Database from 'better-sqlite3';
import type { ScannedPackage } from './connectors/types.js';
import { scanNpm } from './connectors/npm.js';
import { scanPip } from './connectors/pip.js';
import { scanCargo } from './connectors/cargo.js';
import { scanGo } from './connectors/go.js';
import { scanSboms } from './sbom.js';
import { normalizeLicense, classifyLicense } from './license-data.js';

export interface ScanResult {
  scanRunId: string;
  packages: Array<{
    ecosystem: string;
    name: string;
    version: string;
    licenseExpression: string | null;
    licenseCategory: string;
  }>;
  alerts: Array<{
    kind: string;
    severity: string;
    message: string;
  }>;
  error?: string;
}

/**
 * Run a full scan of projectPath, storing results in the provided DB.
 * Updates the scan_run row throughout the process.
 */
export async function runScan(db: Database.Database, scanRunId: string, projectPath: string): Promise<ScanResult> {
  const updateRun = db.prepare(
    `UPDATE scan_runs SET status = ?, started_at = COALESCE(started_at, datetime('now')), error = ? WHERE id = ?`
  );
  const finishRun = db.prepare(
    `UPDATE scan_runs SET status = ?, finished_at = datetime('now'), error = ? WHERE id = ?`
  );

  updateRun.run('running', null, scanRunId);

  try {
    const resolvedPath = path.resolve(projectPath);
    if (!fs.existsSync(resolvedPath)) {
      throw new Error(`Path does not exist: ${resolvedPath}`);
    }

    // Collect packages from all connectors
    const raw: ScannedPackage[] = [];
    raw.push(...scanNpm(resolvedPath));
    raw.push(...scanPip(resolvedPath));
    raw.push(...scanCargo(resolvedPath));
    raw.push(...scanGo(resolvedPath));
    raw.push(...scanSboms(resolvedPath));

    // Upsert packages and record scan results
    const upsertPkg = db.prepare(`
      INSERT INTO packages (id, ecosystem, name, version, license_expression, license_source, updated_at)
      VALUES (@id, @ecosystem, @name, @version, @licenseExpression, @licenseSource, datetime('now'))
      ON CONFLICT(ecosystem, name, version) DO UPDATE SET
        license_expression = COALESCE(excluded.license_expression, license_expression),
        license_source = COALESCE(excluded.license_source, license_source),
        updated_at = datetime('now')
      RETURNING id, license_expression
    `);
    const linkResult = db.prepare(`
      INSERT OR IGNORE INTO scan_results (id, scan_run_id, package_id)
      VALUES (?, ?, ?)
    `);
    const insertAlert = db.prepare(`
      INSERT INTO alerts (id, package_id, scan_run_id, kind, severity, message)
      VALUES (?, ?, ?, ?, ?, ?)
    `);

    const results: ScanResult['packages'] = [];
    const alerts: ScanResult['alerts'] = [];

    const doInserts = db.transaction(() => {
      for (const pkg of raw) {
        const normalized = normalizeLicense(pkg.licenseExpression ?? '');
        const category = classifyLicense(normalized);
        const row = upsertPkg.get({
          id: randomUUID(),
          ecosystem: pkg.ecosystem,
          name: pkg.name,
          version: pkg.version,
          licenseExpression: normalized !== 'NOASSERTION' ? normalized : null,
          licenseSource: pkg.licenseSource ?? null,
        }) as { id: string; license_expression: string | null } | undefined;

        if (!row) continue;
        linkResult.run(randomUUID(), scanRunId, row.id);

        results.push({
          ecosystem: pkg.ecosystem,
          name: pkg.name,
          version: pkg.version,
          licenseExpression: row.license_expression,
          licenseCategory: category,
        });

        // Generate alerts for problematic licenses
        if (category === 'proprietary') {
          const msg = `${pkg.ecosystem}:${pkg.name}@${pkg.version} uses a proprietary license (${row.license_expression ?? 'unknown'})`;
          insertAlert.run(randomUUID(), row.id, scanRunId, 'proprietary-license', 'high', msg);
          alerts.push({ kind: 'proprietary-license', severity: 'high', message: msg });
        } else if (category === 'copyleft-strong') {
          const msg = `${pkg.ecosystem}:${pkg.name}@${pkg.version} uses a strong copyleft license (${row.license_expression ?? 'unknown'})`;
          insertAlert.run(randomUUID(), row.id, scanRunId, 'strong-copyleft', 'medium', msg);
          alerts.push({ kind: 'strong-copyleft', severity: 'medium', message: msg });
        } else if (!row.license_expression) {
          const msg = `${pkg.ecosystem}:${pkg.name}@${pkg.version} has no license information`;
          insertAlert.run(randomUUID(), row.id, scanRunId, 'license-missing', 'low', msg);
          alerts.push({ kind: 'license-missing', severity: 'low', message: msg });
        }
      }
    });

    doInserts();
    finishRun.run('done', null, scanRunId);

    return { scanRunId, packages: results, alerts };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    finishRun.run('failed', message, scanRunId);
    return { scanRunId, packages: [], alerts: [], error: message };
  }
}

/**
 * Create a new scan_run row in the DB and return its ID.
 */
export function createScanRun(db: Database.Database, projectPath: string): string {
  const id = randomUUID();
  db.prepare(
    `INSERT INTO scan_runs (id, project_path, status) VALUES (?, ?, 'pending')`
  ).run(id, projectPath);
  return id;
}
