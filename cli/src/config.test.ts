import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { parseConfig, loadConfig } from './config.js';

// ─── parseConfig ─────────────────────────────────────────────────────────────

describe('parseConfig', () => {
  it('parses a full config', () => {
    const yaml = `
policy: permissive-only
failOnViolation: true
paths:
  - .
  - src/
ecosystem: npm
`;
    expect(parseConfig(yaml)).toEqual({
      policy: 'permissive-only',
      failOnViolation: true,
      paths: ['.', 'src/'],
      ecosystem: 'npm',
    });
  });

  it('strips inline comments', () => {
    const yaml = `policy: no-copyleft-strong  # enforce no GPL
failOnViolation: true  # default CI behavior`;
    const cfg = parseConfig(yaml);
    expect(cfg.policy).toBe('no-copyleft-strong');
    expect(cfg.failOnViolation).toBe(true);
  });

  it('treats ecosystem "auto" as absent', () => {
    const cfg = parseConfig('ecosystem: auto\n');
    expect(cfg.ecosystem).toBeUndefined();
  });

  it('returns empty object for blank input', () => {
    expect(parseConfig('')).toEqual({});
    expect(parseConfig('   \n   \n')).toEqual({});
  });

  it('ignores unknown keys', () => {
    const cfg = parseConfig('unknownKey: value\npolicy: permissive-only\n');
    expect(cfg).toEqual({ policy: 'permissive-only' });
  });

  it('handles failOnViolation: false', () => {
    const cfg = parseConfig('failOnViolation: false\n');
    expect(cfg.failOnViolation).toBe(false);
  });

  it('handles paths list that ends at EOF', () => {
    const cfg = parseConfig('paths:\n  - .\n  - lib/\n');
    expect(cfg.paths).toEqual(['.', 'lib/']);
  });

  it('ends a paths sequence when a new key is found', () => {
    const yaml = `paths:\n  - .\npolicy: permissive-only\n`;
    const cfg = parseConfig(yaml);
    expect(cfg.paths).toEqual(['.']);
    expect(cfg.policy).toBe('permissive-only');
  });
});

// ─── loadConfig ──────────────────────────────────────────────────────────────

describe('loadConfig', () => {
  let tmpDir: string;

  beforeEach(() => {
    tmpDir = join(tmpdir(), `phalus-config-test-${Date.now()}`);
    mkdirSync(tmpDir, { recursive: true });
  });

  afterEach(() => {
    rmSync(tmpDir, { recursive: true, force: true });
  });

  it('returns empty config when .phalus.yml does not exist', () => {
    expect(loadConfig(tmpDir)).toEqual({});
  });

  it('loads and parses .phalus.yml from the given directory', () => {
    writeFileSync(join(tmpDir, '.phalus.yml'), 'policy: no-proprietary\nfailOnViolation: true\n');
    expect(loadConfig(tmpDir)).toEqual({ policy: 'no-proprietary', failOnViolation: true });
  });

  it('returns empty config when .phalus.yml is invalid (graceful fallback)', () => {
    writeFileSync(join(tmpDir, '.phalus.yml'), Buffer.from([0xff, 0xfe])); // binary garbage
    expect(loadConfig(tmpDir)).toEqual({});
  });
});
