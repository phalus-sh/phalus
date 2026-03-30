import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import Database from 'better-sqlite3';
import { CREATE_TABLES_SQL } from './schema.js';
import { createScanRun, runScan } from './scanner.js';

let tmpDir: string;
let db: Database.Database;

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-scan-'));
  db = new Database(':memory:');
  db.pragma('foreign_keys = ON');
  db.exec(CREATE_TABLES_SQL);
});

afterEach(() => {
  db.close();
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('runScan', () => {
  it('returns error for non-existent path', async () => {
    const scanRunId = createScanRun(db, '/nonexistent/path');
    const result = await runScan(db, scanRunId, '/nonexistent/path');
    expect(result.error).toBeDefined();
    expect(result.packages).toHaveLength(0);
  });

  it('scans an npm project and stores results', async () => {
    fs.writeFileSync(path.join(tmpDir, 'package.json'), JSON.stringify({
      name: 'test',
      dependencies: { 'lodash': '^4.17.21' },
    }));
    const scanRunId = createScanRun(db, tmpDir);
    const result = await runScan(db, scanRunId, tmpDir);
    expect(result.error).toBeUndefined();
    expect(result.packages.length).toBeGreaterThan(0);
    const lodash = result.packages.find(p => p.name === 'lodash');
    expect(lodash).toBeDefined();
    expect(lodash!.ecosystem).toBe('npm');
  });

  it('generates alerts for packages missing license info', async () => {
    fs.writeFileSync(path.join(tmpDir, 'package.json'), JSON.stringify({
      name: 'test',
      dependencies: { 'mystery-pkg': '1.0.0' },
    }));
    const scanRunId = createScanRun(db, tmpDir);
    const result = await runScan(db, scanRunId, tmpDir);
    // mystery-pkg has no license — should get a license-missing alert
    const alert = result.alerts.find(a => a.kind === 'license-missing');
    expect(alert).toBeDefined();
  });

  it('marks scan_run as done on success', async () => {
    fs.writeFileSync(path.join(tmpDir, 'package.json'), JSON.stringify({ name: 'test', dependencies: {} }));
    const scanRunId = createScanRun(db, tmpDir);
    await runScan(db, scanRunId, tmpDir);
    const run = db.prepare('SELECT status FROM scan_runs WHERE id = ?').get(scanRunId) as { status: string };
    expect(run.status).toBe('done');
  });
});
