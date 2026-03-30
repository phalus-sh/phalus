import path from 'node:path';
import { Router, type IRouter, type Request, type Response, type NextFunction } from 'express';
import { z } from 'zod';
import { getDb, createScanRun, runScan } from '@phalus/core';
import { scanRateLimit } from '../middleware/rate-limit.js';

export const scansRouter: IRouter = Router();

// Optional base directory constraint — if set, all scan paths must resolve within it.
const SCAN_BASE_DIR = process.env['PHALUS_SCAN_BASE_DIR']
  ? path.resolve(process.env['PHALUS_SCAN_BASE_DIR'])
  : null;

const ScanRequestSchema = z.object({
  path: z
    .string({ required_error: '`path` is required' })
    .min(1, '`path` cannot be empty')
    .refine(p => !p.split(/[\\/]/).some(seg => seg === '..'), {
      message: 'Path traversal sequences are not allowed',
    }),
  policyId: z.string().optional(),
});

/**
 * Enforce Content-Type: application/json on mutation endpoints.
 */
function requireJsonContentType(req: Request, res: Response, next: NextFunction): void {
  const ct = req.headers['content-type'] ?? '';
  if (!ct.includes('application/json')) {
    res.status(415).json({ error: 'Content-Type must be application/json' });
    return;
  }
  next();
}

/**
 * Strip absolute filesystem paths from error messages to avoid leaking internal structure.
 */
function sanitizeErrorMessage(message: string): string {
  return message
    .replace(/\/[^\s,'"]+/g, '<path>')
    .replace(/[A-Za-z]:\\[^\s,'"]+/g, '<path>');
}

/**
 * POST /scans
 * Body: { "path": "/absolute/or/relative/project/path", "policyId"?: "policy-id-or-name" }
 * Triggers a scan synchronously and returns the result.
 */
scansRouter.post('/', scanRateLimit, requireJsonContentType, async (req, res) => {
  const parsed = ScanRequestSchema.safeParse(req.body);
  if (!parsed.success) {
    res.status(400).json({ error: parsed.error.errors[0]?.message ?? 'Invalid request' });
    return;
  }

  const { path: projectPath, policyId } = parsed.data;
  const resolved = path.resolve(projectPath);

  // Enforce base directory restriction when configured
  if (SCAN_BASE_DIR && !resolved.startsWith(SCAN_BASE_DIR + path.sep) && resolved !== SCAN_BASE_DIR) {
    res.status(400).json({ error: 'Path is outside the allowed scan directory' });
    return;
  }

  const db = getDb();
  const scanRunId = createScanRun(db, projectPath);
  const result = await runScan(db, scanRunId, projectPath, policyId);

  if (result.error) {
    res.status(500).json({
      scanRunId: result.scanRunId,
      packages: [],
      alerts: [],
      error: sanitizeErrorMessage(result.error),
    });
    return;
  }

  res.status(200).json(result);
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
