import { describe, it, expect, afterAll } from 'vitest';
import request from 'supertest';
import os from 'node:os';
import path from 'node:path';
import fs from 'node:fs';

// Set DB path before the app module loads
const tmpDb = path.join(os.tmpdir(), `phalus-test-${Date.now()}.db`);
process.env['PHALUS_DB_PATH'] = tmpDb;

import app from './index.js';

afterAll(() => {
  if (fs.existsSync(tmpDb)) fs.unlinkSync(tmpDb);
});

describe('GET /health', () => {
  it('returns 200 with status ok', async () => {
    const res = await request(app).get('/health');
    expect(res.status).toBe(200);
    expect(res.body.status).toBe('ok');
  });
});

describe('GET /openapi.json', () => {
  it('returns a valid OpenAPI spec', async () => {
    const res = await request(app).get('/openapi.json');
    expect(res.status).toBe(200);
    expect(res.body.openapi).toBe('3.1.0');
  });
});

describe('POST /scans', () => {
  it('returns 400 when path is missing', async () => {
    const res = await request(app).post('/scans').send({});
    expect(res.status).toBe(400);
  });

  it('scans a valid directory and returns scan results', async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-api-test-'));
    fs.writeFileSync(path.join(tmpDir, 'package.json'), JSON.stringify({
      name: 'test',
      dependencies: { 'express': '^4.18.0' },
    }));
    try {
      const res = await request(app).post('/scans').send({ path: tmpDir });
      expect(res.status).toBe(200);
      expect(res.body.scanRunId).toBeDefined();
      expect(Array.isArray(res.body.packages)).toBe(true);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });
});

describe('GET /licenses', () => {
  it('returns a license list', async () => {
    const res = await request(app).get('/licenses');
    expect(res.status).toBe(200);
    expect(Array.isArray(res.body.licenses)).toBe(true);
  });
});
