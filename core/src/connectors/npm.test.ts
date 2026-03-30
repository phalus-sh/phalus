import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { scanNpm } from './npm.js';

let tmpDir: string;

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-npm-'));
});

afterEach(() => {
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('scanNpm', () => {
  it('returns empty array when no manifests', () => {
    expect(scanNpm(tmpDir)).toEqual([]);
  });

  it('parses package.json dependencies', () => {
    fs.writeFileSync(path.join(tmpDir, 'package.json'), JSON.stringify({
      name: 'test',
      dependencies: { 'lodash': '^4.17.21', 'express': '~4.18.0' },
      devDependencies: { 'vitest': '^1.5.0' },
    }));
    const pkgs = scanNpm(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('lodash');
    expect(pkgs.map(p => p.name)).toContain('express');
    expect(pkgs.map(p => p.name)).toContain('vitest');
    const lodash = pkgs.find(p => p.name === 'lodash')!;
    expect(lodash.version).toBe('4.17.21'); // range prefix stripped
    expect(lodash.ecosystem).toBe('npm');
  });

  it('prefers package-lock.json over package.json', () => {
    fs.writeFileSync(path.join(tmpDir, 'package.json'), JSON.stringify({
      dependencies: { 'lodash': '^4.17.21' },
    }));
    fs.writeFileSync(path.join(tmpDir, 'package-lock.json'), JSON.stringify({
      lockfileVersion: 2,
      packages: {
        'node_modules/lodash': { version: '4.17.21', license: 'MIT' },
      },
    }));
    const pkgs = scanNpm(tmpDir);
    expect(pkgs).toHaveLength(1);
    expect(pkgs[0]!.version).toBe('4.17.21');
    expect(pkgs[0]!.licenseExpression).toBe('MIT');
  });
});
