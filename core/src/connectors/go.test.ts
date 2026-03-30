import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { scanGo } from './go.js';

let tmpDir: string;

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-go-'));
});

afterEach(() => {
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('scanGo', () => {
  it('returns empty array when no go.mod', () => {
    expect(scanGo(tmpDir)).toEqual([]);
  });

  it('parses go.mod require block', () => {
    fs.writeFileSync(path.join(tmpDir, 'go.mod'), `
module example.com/myapp

go 1.21

require (
  github.com/gin-gonic/gin v1.9.1
  golang.org/x/crypto v0.17.0 // indirect
)
`);
    const pkgs = scanGo(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('github.com/gin-gonic/gin');
    expect(pkgs.map(p => p.name)).toContain('golang.org/x/crypto');
    const gin = pkgs.find(p => p.name === 'github.com/gin-gonic/gin')!;
    expect(gin.version).toBe('v1.9.1');
    expect(gin.ecosystem).toBe('go');
  });

  it('strips +incompatible suffix', () => {
    fs.writeFileSync(path.join(tmpDir, 'go.mod'), `
module example.com/myapp
go 1.21
require github.com/some/old v1.0.0+incompatible
`);
    const pkgs = scanGo(tmpDir);
    expect(pkgs[0]!.version).toBe('v1.0.0');
  });
});
