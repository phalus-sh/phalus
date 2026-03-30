import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { scanRuby } from './ruby.js';

let tmpDir: string;

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-ruby-'));
});

afterEach(() => {
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('scanRuby', () => {
  it('returns empty array when no Gemfile.lock', () => {
    expect(scanRuby(tmpDir)).toEqual([]);
  });

  it('parses GEM section specs', () => {
    fs.writeFileSync(path.join(tmpDir, 'Gemfile.lock'), `
GEM
  remote: https://rubygems.org/
  specs:
    rails (7.0.4)
      actioncable (= 7.0.4)
      actionmailer (= 7.0.4)
    actioncable (7.0.4)
      actionpack (= 7.0.4)
    actionmailer (7.0.4)

BUNDLED WITH
   2.3.26
`);
    const pkgs = scanRuby(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('rails');
    expect(pkgs.map(p => p.name)).toContain('actioncable');
    expect(pkgs.map(p => p.name)).toContain('actionmailer');
    const rails = pkgs.find(p => p.name === 'rails')!;
    expect(rails.version).toBe('7.0.4');
    expect(rails.ecosystem).toBe('ruby');
  });

  it('parses PATH section gems', () => {
    fs.writeFileSync(path.join(tmpDir, 'Gemfile.lock'), `
PATH
  remote: .
  specs:
    myapp (0.1.0)
      rails (~> 7.0)

GEM
  remote: https://rubygems.org/
  specs:
    rails (7.0.4)

BUNDLED WITH
   2.3.26
`);
    const pkgs = scanRuby(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('myapp');
    expect(pkgs.map(p => p.name)).toContain('rails');
    const myapp = pkgs.find(p => p.name === 'myapp')!;
    expect(myapp.version).toBe('0.1.0');
  });

  it('parses GIT section gems', () => {
    fs.writeFileSync(path.join(tmpDir, 'Gemfile.lock'), `
GIT
  remote: https://github.com/user/mygem.git
  revision: abc123
  specs:
    mygem (2.0.0)

GEM
  remote: https://rubygems.org/
  specs:
    rake (13.0.6)

BUNDLED WITH
   2.3.26
`);
    const pkgs = scanRuby(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('mygem');
    expect(pkgs.map(p => p.name)).toContain('rake');
    const mygem = pkgs.find(p => p.name === 'mygem')!;
    expect(mygem.version).toBe('2.0.0');
  });

  it('deduplicates gems appearing in multiple sections', () => {
    fs.writeFileSync(path.join(tmpDir, 'Gemfile.lock'), `
GEM
  remote: https://rubygems.org/
  specs:
    rake (13.0.6)
    rake (13.0.6)

BUNDLED WITH
   2.3.26
`);
    const pkgs = scanRuby(tmpDir);
    const rakes = pkgs.filter(p => p.name === 'rake');
    expect(rakes).toHaveLength(1);
  });
});
