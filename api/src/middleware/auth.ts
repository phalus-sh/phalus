import { timingSafeEqual } from 'node:crypto';
import type { Request, Response, NextFunction } from 'express';

const API_KEY = process.env['PHALUS_API_KEY'];

/**
 * Bearer token auth middleware.
 * Skipped entirely when PHALUS_API_KEY is not set (dev / open mode).
 *
 * Uses constant-time comparison to prevent timing attacks.
 * Returns 401 for both missing and invalid credentials (no auth info vs bad auth info).
 */
export function requireApiKey(req: Request, res: Response, next: NextFunction): void {
  if (!API_KEY) {
    // No key configured — open mode, skip auth
    next();
    return;
  }
  const header = req.headers['authorization'] ?? '';
  if (!header.startsWith('Bearer ')) {
    // No credentials provided
    res.status(401).json({ error: 'Unauthorized' });
    return;
  }
  const token = header.slice(7);

  // Constant-time comparison: pad both buffers to the same length so timingSafeEqual
  // doesn't reject mismatched lengths. We then also check original lengths match.
  const keyBuf = Buffer.from(API_KEY, 'utf8');
  const len = Math.max(keyBuf.length, Buffer.byteLength(token, 'utf8'));
  const a = Buffer.alloc(len, 0);
  const b = Buffer.alloc(len, 0);
  keyBuf.copy(a);
  Buffer.from(token, 'utf8').copy(b);

  const valid = timingSafeEqual(a, b) && keyBuf.length === Buffer.byteLength(token, 'utf8');
  if (!valid) {
    res.status(401).json({ error: 'Unauthorized' });
    return;
  }
  next();
}
