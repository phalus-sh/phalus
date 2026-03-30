import type { Request, Response, NextFunction } from 'express';

const API_KEY = process.env['PHALUS_API_KEY'];

/**
 * Bearer token auth middleware.
 * Skipped entirely when PHALUS_API_KEY is not set (dev / open mode).
 */
export function requireApiKey(req: Request, res: Response, next: NextFunction): void {
  if (!API_KEY) {
    // No key configured — open mode, skip auth
    next();
    return;
  }
  const header = req.headers['authorization'] ?? '';
  const token = header.startsWith('Bearer ') ? header.slice(7) : '';
  if (token !== API_KEY) {
    res.status(401).json({ error: 'Unauthorized' });
    return;
  }
  next();
}
