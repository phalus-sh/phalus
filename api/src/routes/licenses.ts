import { Router, type IRouter } from 'express';
import { getDb, classifyLicense } from '@phalus/core';

export const licensesRouter: IRouter = Router();

/**
 * GET /licenses
 * Query params:
 *   q         — search by name or license expression (substring, case-insensitive)
 *   ecosystem — filter by ecosystem (npm, pip, cargo, go, …)
 *   category  — filter by license category (permissive, copyleft-weak, copyleft-strong, proprietary, unknown)
 *   limit     — max results (default 100, max 500)
 *   offset    — pagination offset
 */
licensesRouter.get('/', (req, res) => {
  const db = getDb();
  const { q, ecosystem, category, limit: limitQ, offset: offsetQ } = req.query as Record<string, string | undefined>;
  const limit = Math.min(Number(limitQ ?? 100), 500);
  const offset = Number(offsetQ ?? 0);

  const conditions: string[] = [];
  const params: unknown[] = [];

  if (q) {
    conditions.push(`(LOWER(p.name) LIKE LOWER(?) OR LOWER(p.license_expression) LIKE LOWER(?))`);
    const like = `%${q}%`;
    params.push(like, like);
  }
  if (ecosystem) {
    conditions.push(`p.ecosystem = ?`);
    params.push(ecosystem);
  }

  const where = conditions.length > 0 ? `WHERE ${conditions.join(' AND ')}` : '';
  const rows = db.prepare(`
    SELECT p.id, p.ecosystem, p.name, p.version, p.license_expression, p.license_source, p.created_at, p.updated_at
    FROM packages p
    ${where}
    ORDER BY p.ecosystem, p.name, p.version
    LIMIT ? OFFSET ?
  `).all(...params, limit, offset) as Array<{
    id: string;
    ecosystem: string;
    name: string;
    version: string;
    license_expression: string | null;
    license_source: string | null;
    created_at: string;
    updated_at: string;
  }>;

  // Attach classification and optionally filter by category
  let result = rows.map(r => ({
    ...r,
    licenseCategory: classifyLicense(r.license_expression ?? ''),
  }));
  if (category) {
    result = result.filter(r => r.licenseCategory === category);
  }

  res.json({ total: result.length, limit, offset, licenses: result });
});
