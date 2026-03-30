import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { scanPhp } from './php.js';

let tmpDir: string;

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-php-'));
});

afterEach(() => {
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('scanPhp', () => {
  it('returns empty array when no composer.lock', () => {
    expect(scanPhp(tmpDir)).toEqual([]);
  });

  it('parses packages with license array', () => {
    fs.writeFileSync(path.join(tmpDir, 'composer.lock'), JSON.stringify({
      packages: [
        {
          name: 'symfony/console',
          version: '6.3.0',
          license: ['MIT'],
        },
        {
          name: 'doctrine/orm',
          version: '2.15.0',
          license: ['MIT'],
        },
      ],
      'packages-dev': [],
    }));
    const pkgs = scanPhp(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('symfony/console');
    expect(pkgs.map(p => p.name)).toContain('doctrine/orm');
    const symfony = pkgs.find(p => p.name === 'symfony/console')!;
    expect(symfony.version).toBe('6.3.0');
    expect(symfony.ecosystem).toBe('php');
    expect(symfony.licenseExpression).toBe('MIT');
    expect(symfony.licenseSource).toBe('composer.lock');
  });

  it('parses packages-dev section', () => {
    fs.writeFileSync(path.join(tmpDir, 'composer.lock'), JSON.stringify({
      packages: [],
      'packages-dev': [
        {
          name: 'phpunit/phpunit',
          version: '10.0.0',
          license: ['BSD-3-Clause'],
        },
      ],
    }));
    const pkgs = scanPhp(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('phpunit/phpunit');
    const phpunit = pkgs.find(p => p.name === 'phpunit/phpunit')!;
    expect(phpunit.licenseExpression).toBe('BSD-3-Clause');
  });

  it('joins multiple licenses with OR', () => {
    fs.writeFileSync(path.join(tmpDir, 'composer.lock'), JSON.stringify({
      packages: [
        {
          name: 'some/dual-licensed',
          version: '1.0.0',
          license: ['MIT', 'Apache-2.0'],
        },
      ],
      'packages-dev': [],
    }));
    const pkgs = scanPhp(tmpDir);
    const pkg = pkgs.find(p => p.name === 'some/dual-licensed')!;
    expect(pkg.licenseExpression).toBe('MIT OR Apache-2.0');
  });

  it('strips v prefix from versions', () => {
    fs.writeFileSync(path.join(tmpDir, 'composer.lock'), JSON.stringify({
      packages: [
        { name: 'vendor/pkg', version: 'v2.1.0', license: ['MIT'] },
      ],
      'packages-dev': [],
    }));
    const pkgs = scanPhp(tmpDir);
    expect(pkgs[0]!.version).toBe('2.1.0');
  });

  it('handles missing license gracefully', () => {
    fs.writeFileSync(path.join(tmpDir, 'composer.lock'), JSON.stringify({
      packages: [
        { name: 'vendor/nolicense', version: '1.0.0' },
      ],
      'packages-dev': [],
    }));
    const pkgs = scanPhp(tmpDir);
    expect(pkgs[0]!.licenseExpression).toBeUndefined();
  });
});
