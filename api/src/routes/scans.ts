import { Router, type IRouter } from 'express';
import { getDb, createScanRun, runScan } from '@phalus/core';

export const scansRouter: IRouter = Router();

/**
 * POST /scans
 * Body: { "path": "/absolute/or/relative/project/path" }
 * Triggers a scan synchronously and returns the result.
 */
scansRouter.post('/', async (req, res) => {
  const { path: projectPath } = req.body as { path?: string };
  if (!projectPath || typeof projectPath !== 'string') {
    res.status(400).json({ error: '`path` is required' });
    return;
  }
  const db = getDb();
  const scanRunId = createScanRun(db, projectPath);
  const result = await runScan(db, scanRunId, projectPath);
  const status = result.error ? 500 : 200;
  res.status(status).json(result);
});

/**
 * GET /scans/:id
 * Returns scan_run details plus its packages and alerts.
 */
scansRouter.get('/:id', (req, res) => {
  const db = getDb();
  const run = db.prepare(`SELECT * FROM scan_runs WHERE id = ?`).get(req.params['id']) as Record<string, unknown> | undefined;
  if (!run) {
    res.status(404).json({ error: 'Scan not found' });
    return;
  }
  const packages = db.prepare(`
    SELECT p.*, sr.scan_run_id
    FROM packages p
    JOIN scan_results sr ON sr.package_id = p.id
    WHERE sr.scan_run_id = ?
  `).all(req.params['id']);
  const alerts = db.prepare(`SELECT * FROM alerts WHERE scan_run_id = ?`).all(req.params['id']);
  res.json({ ...run, packages, alerts });
});

/**
 * GET /scans
 * Lists all scan runs (most recent first, limit 50).
 */
scansRouter.get('/', (req, res) => {
  const db = getDb();
  const limit = Math.min(Number(req.query['limit'] ?? 50), 200);
  const runs = db.prepare(`SELECT * FROM scan_runs ORDER BY created_at DESC LIMIT ?`).all(limit);
  res.json({ runs });
});
