import rateLimit from 'express-rate-limit';
import type { Request, Response, NextFunction } from 'express';

// Disable rate limiting in test environments via PHALUS_RATE_LIMIT=false
const RATE_LIMIT_ENABLED = process.env['PHALUS_RATE_LIMIT'] !== 'false';

const noop = (_req: Request, _res: Response, next: NextFunction): void => next();

/**
 * Keyed by API key (when present) or IP, so limits are per-client.
 */
function makeKeyGenerator(req: Request): string {
  const auth = req.headers['authorization'] ?? '';
  const token = auth.startsWith('Bearer ') ? auth.slice(7) : '';
  return token || req.ip || 'unknown';
}

/**
 * POST /scans — 10 requests per minute per client.
 */
export const scanRateLimit = RATE_LIMIT_ENABLED
  ? rateLimit({
      windowMs: 60 * 1000,
      max: 10,
      keyGenerator: makeKeyGenerator,
      standardHeaders: true,
      legacyHeaders: false,
      message: { error: 'Too many requests', retryAfter: 60 },
    })
  : noop;

/**
 * All other routes — 60 requests per minute per client.
 */
export const defaultRateLimit = RATE_LIMIT_ENABLED
  ? rateLimit({
      windowMs: 60 * 1000,
      max: 60,
      keyGenerator: makeKeyGenerator,
      standardHeaders: true,
      legacyHeaders: false,
      message: { error: 'Too many requests', retryAfter: 60 },
    })
  : noop;
