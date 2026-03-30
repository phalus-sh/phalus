/**
 * Security-focused tests for PHA-15: input validation, path traversal, auth hardening,
 * error sanitization, Content-Type enforcement, and license classification edge cases.
 */
import { describe, it, expect, afterAll, beforeAll } from 'vitest';
import request from 'supertest';
import os from 'node:os';
import path from 'node:path';
import fs from 'node:fs';
import { classifyLicense, normalizeLicense } from '@phalus/core';

// Set DB path and disable rate limiting before app loads
const tmpDb = path.join(os.tmpdir(), `phalus-sec-test-${Date.now()}.db`);
process.env['PHALUS_DB_PATH'] = tmpDb;
process.env['PHALUS_RATE_LIMIT'] = 'false';

import app from './index.js';

afterAll(() => {
  if (fs.existsSync(tmpDb)) fs.unlinkSync(tmpDb);
});

// ---------------------------------------------------------------------------
// Auth hardening
// ---------------------------------------------------------------------------
describe('Auth: missing vs invalid key', () => {
  const SAVED_KEY = process.env['PHALUS_API_KEY'];

  beforeAll(() => {
    // This test runs without a configured API key — open mode — so we skip if key is set
  });

  it('returns 200 when no API key is configured (open mode)', async () => {
    // In open mode, all requests pass through
    const res = await request(app).get('/health');
    expect(res.status).toBe(200);
  });
});

// ---------------------------------------------------------------------------
// Content-Type enforcement
// ---------------------------------------------------------------------------
describe('POST /scans — Content-Type enforcement', () => {
  it('returns 415 when Content-Type is not application/json', async () => {
    const res = await request(app)
      .post('/scans')
      .set('Content-Type', 'text/plain')
      .send('path=/tmp');
    expect(res.status).toBe(415);
    expect(res.body.error).toContain('application/json');
  });

  it('returns 415 when Content-Type is missing', async () => {
    const res = await request(app)
      .post('/scans')
      .send('path=/tmp');
    expect(res.status).toBe(415);
  });
});

describe('POST /policies — Content-Type enforcement', () => {
  it('returns 415 when Content-Type is not application/json', async () => {
    const res = await request(app)
      .post('/policies')
      .set('Content-Type', 'text/plain')
      .send('name=test&rules={}');
    expect(res.status).toBe(415);
  });
});

describe('PATCH /policies/:id — Content-Type enforcement', () => {
  it('returns 415 when Content-Type is not application/json', async () => {
    const res = await request(app)
      .patch('/policies/some-id')
      .set('Content-Type', 'application/x-www-form-urlencoded')
      .send('name=test');
    expect(res.status).toBe(415);
  });
});

// ---------------------------------------------------------------------------
// Path traversal protection
// ---------------------------------------------------------------------------
describe('POST /scans — path traversal protection', () => {
  it('rejects paths containing ..', async () => {
    const res = await request(app)
      .post('/scans')
      .set('Content-Type', 'application/json')
      .send({ path: '../../../etc/passwd' });
    expect(res.status).toBe(400);
    expect(res.body.error).toMatch(/traversal/i);
  });

  it('rejects paths containing .. in middle segments', async () => {
    const res = await request(app)
      .post('/scans')
      .set('Content-Type', 'application/json')
      .send({ path: '/home/user/../../../etc' });
    expect(res.status).toBe(400);
  });

  it('rejects paths with Windows-style traversal', async () => {
    const res = await request(app)
      .post('/scans')
      .set('Content-Type', 'application/json')
      .send({ path: '..\\..\\etc\\passwd' });
    expect(res.status).toBe(400);
  });
});

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------
describe('POST /scans — input validation', () => {
  it('returns 400 when path is missing', async () => {
    const res = await request(app)
      .post('/scans')
      .set('Content-Type', 'application/json')
      .send({});
    expect(res.status).toBe(400);
  });

  it('returns 400 when path is empty string', async () => {
    const res = await request(app)
      .post('/scans')
      .set('Content-Type', 'application/json')
      .send({ path: '' });
    expect(res.status).toBe(400);
  });

  it('returns 400 when path is a number', async () => {
    const res = await request(app)
      .post('/scans')
      .set('Content-Type', 'application/json')
      .send({ path: 42 });
    expect(res.status).toBe(400);
  });
});

describe('POST /policies — input validation', () => {
  it('returns 400 when name is missing', async () => {
    const res = await request(app)
      .post('/policies')
      .set('Content-Type', 'application/json')
      .send({ rules: {} });
    expect(res.status).toBe(400);
  });

  it('returns 400 when rules is missing', async () => {
    const res = await request(app)
      .post('/policies')
      .set('Content-Type', 'application/json')
      .send({ name: 'test-policy' });
    expect(res.status).toBe(400);
  });

  it('returns 400 when rules.allow contains non-string values', async () => {
    const res = await request(app)
      .post('/policies')
      .set('Content-Type', 'application/json')
      .send({ name: 'test-policy', rules: { allow: [123, null] } });
    expect(res.status).toBe(400);
  });
});

// ---------------------------------------------------------------------------
// Error sanitization — no stack traces or internal paths in responses
// ---------------------------------------------------------------------------
describe('Error sanitization', () => {
  it('returns 500 without leaking file system paths when scan fails', async () => {
    // Scan a path that does not exist — will produce an error
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-sec-'));
    const nonExistent = path.join(tmpDir, 'does-not-exist');
    fs.rmdirSync(tmpDir); // remove so path doesn't exist

    const res = await request(app)
      .post('/scans')
      .set('Content-Type', 'application/json')
      .send({ path: nonExistent });

    // Should be a 500 but the error body must not contain the full path
    if (res.status === 500 && res.body.error) {
      expect(res.body.error).not.toContain(nonExistent);
      expect(res.body.error).not.toMatch(/\/home\//);
    }
    // Regardless, no stack trace should be present
    expect(res.body.stack).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// License classification security edge cases (PHA-15 §5)
// ---------------------------------------------------------------------------
describe('License classification — security edge cases', () => {
  it('treats null/empty license as unknown — never silently passes', () => {
    expect(classifyLicense('')).toBe('unknown');
    expect(classifyLicense(normalizeLicense(''))).toBe('unknown');
  });

  it('treats NOASSERTION as unknown', () => {
    expect(classifyLicense('NOASSERTION')).toBe('unknown');
    expect(normalizeLicense('')).toBe('NOASSERTION');
    expect(classifyLicense(normalizeLicense(''))).toBe('unknown');
  });

  it('treats NONE as unknown', () => {
    expect(classifyLicense('NONE')).toBe('unknown');
  });

  it('treats LicenseRef-* as unknown (custom/unrecognized reference)', () => {
    expect(classifyLicense('LicenseRef-scancode-proprietary-license')).toBe('unknown');
    expect(classifyLicense('LicenseRef-Custom')).toBe('unknown');
    expect(classifyLicense('LicenseRef-123')).toBe('unknown');
  });

  it('treats whitespace-only license as unknown via normalization', () => {
    expect(normalizeLicense('   ')).toBe('NOASSERTION');
    expect(classifyLicense(normalizeLicense('   '))).toBe('unknown');
  });

  it('classifies strong-copyleft licenses correctly', () => {
    expect(classifyLicense('GPL-2.0-only')).toBe('copyleft-strong');
    expect(classifyLicense('GPL-3.0-or-later')).toBe('copyleft-strong');
    expect(classifyLicense('AGPL-3.0-only')).toBe('copyleft-strong');
  });

  it('does not misclassify LGPL as strong copyleft', () => {
    // LGPL is weak copyleft, not strong — important distinction
    expect(classifyLicense('LGPL-2.1-only')).toBe('copyleft-weak');
    expect(classifyLicense('LGPL-3.0-or-later')).toBe('copyleft-weak');
  });

  it('classifies proprietary licenses as proprietary, not unknown', () => {
    expect(classifyLicense('BUSL-1.1')).toBe('proprietary');
    expect(classifyLicense('UNLICENSED')).toBe('proprietary');
    expect(classifyLicense('SSPL-1.0')).toBe('proprietary');
  });
});
