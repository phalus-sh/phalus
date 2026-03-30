import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { scanNuget } from './nuget.js';

let tmpDir: string;

beforeEach(() => {
  tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'phalus-nuget-'));
});

afterEach(() => {
  fs.rmSync(tmpDir, { recursive: true, force: true });
});

describe('scanNuget', () => {
  it('returns empty array when no manifest', () => {
    expect(scanNuget(tmpDir)).toEqual([]);
  });

  it('parses packages.lock.json', () => {
    fs.writeFileSync(path.join(tmpDir, 'packages.lock.json'), JSON.stringify({
      version: 1,
      dependencies: {
        'net6.0': {
          'Newtonsoft.Json': {
            type: 'Direct',
            requested: '[13.0.1, )',
            resolved: '13.0.1',
            contentHash: 'abc',
          },
          'Microsoft.Extensions.Logging': {
            type: 'Transitive',
            requested: '[6.0.0, )',
            resolved: '6.0.0',
            contentHash: 'def',
          },
        },
      },
    }));
    const pkgs = scanNuget(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('Newtonsoft.Json');
    expect(pkgs.map(p => p.name)).toContain('Microsoft.Extensions.Logging');
    const nj = pkgs.find(p => p.name === 'Newtonsoft.Json')!;
    expect(nj.version).toBe('13.0.1');
    expect(nj.ecosystem).toBe('nuget');
  });

  it('deduplicates packages across TFMs in packages.lock.json', () => {
    fs.writeFileSync(path.join(tmpDir, 'packages.lock.json'), JSON.stringify({
      version: 1,
      dependencies: {
        'net6.0': {
          'Newtonsoft.Json': { type: 'Direct', resolved: '13.0.1' },
        },
        'net7.0': {
          'Newtonsoft.Json': { type: 'Direct', resolved: '13.0.1' },
        },
      },
    }));
    const pkgs = scanNuget(tmpDir);
    const njs = pkgs.filter(p => p.name === 'Newtonsoft.Json');
    expect(njs).toHaveLength(1);
  });

  it('falls back to .csproj PackageReference (self-closing)', () => {
    fs.writeFileSync(path.join(tmpDir, 'MyApp.csproj'), `
<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.1" />
    <PackageReference Include="Serilog" Version="2.12.0" />
  </ItemGroup>
</Project>
`);
    const pkgs = scanNuget(tmpDir);
    expect(pkgs.map(p => p.name)).toContain('Newtonsoft.Json');
    expect(pkgs.map(p => p.name)).toContain('Serilog');
    const nj = pkgs.find(p => p.name === 'Newtonsoft.Json')!;
    expect(nj.version).toBe('13.0.1');
    expect(nj.ecosystem).toBe('nuget');
  });

  it('parses multi-line PackageReference in .csproj', () => {
    fs.writeFileSync(path.join(tmpDir, 'MyApp.csproj'), `
<Project Sdk="Microsoft.NET.Sdk">
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json">
      <Version>13.0.1</Version>
    </PackageReference>
  </ItemGroup>
</Project>
`);
    const pkgs = scanNuget(tmpDir);
    const nj = pkgs.find(p => p.name === 'Newtonsoft.Json')!;
    expect(nj.version).toBe('13.0.1');
  });

  it('prefers packages.lock.json over .csproj', () => {
    fs.writeFileSync(path.join(tmpDir, 'packages.lock.json'), JSON.stringify({
      version: 1,
      dependencies: {
        'net6.0': {
          'Newtonsoft.Json': { type: 'Direct', resolved: '13.0.1' },
        },
      },
    }));
    fs.writeFileSync(path.join(tmpDir, 'MyApp.csproj'), `
<Project>
  <ItemGroup>
    <PackageReference Include="SomeOther" Version="1.0.0" />
  </ItemGroup>
</Project>
`);
    const pkgs = scanNuget(tmpDir);
    // Should only have packages.lock.json content
    expect(pkgs.map(p => p.name)).toContain('Newtonsoft.Json');
    expect(pkgs.map(p => p.name)).not.toContain('SomeOther');
  });
});
