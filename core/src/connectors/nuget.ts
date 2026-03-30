import fs from 'node:fs';
import path from 'node:path';
import type { ScannedPackage } from './types.js';

/**
 * Parse .NET/NuGet manifests.
 * Prefers packages.lock.json (authoritative resolved versions);
 * falls back to scanning *.csproj <PackageReference> elements.
 */
export function scanNuget(dir: string): ScannedPackage[] {
  const lockPath = path.join(dir, 'packages.lock.json');
  if (fs.existsSync(lockPath)) {
    return parseNugetLock(lockPath);
  }
  return parseCsprojFiles(dir);
}

function parseNugetLock(lockPath: string): ScannedPackage[] {
  const raw = JSON.parse(fs.readFileSync(lockPath, 'utf8')) as Record<string, unknown>;
  const packages: ScannedPackage[] = [];
  const seen = new Set<string>();

  // dependencies is keyed by TFM (e.g. "net6.0"), values are package maps
  const deps = raw['dependencies'];
  if (!deps || typeof deps !== 'object') return [];

  for (const tfmDeps of Object.values(deps as Record<string, Record<string, unknown>>)) {
    if (!tfmDeps || typeof tfmDeps !== 'object') continue;
    for (const [pkgName, pkgMeta] of Object.entries(tfmDeps)) {
      if (!pkgMeta || typeof pkgMeta !== 'object') continue;
      const meta = pkgMeta as Record<string, unknown>;
      const version = typeof meta['resolved'] === 'string' ? meta['resolved'] : 'unknown';
      const key = `${pkgName}@${version}`;
      if (!seen.has(key)) {
        seen.add(key);
        packages.push({ ecosystem: 'nuget', name: pkgName, version });
      }
    }
  }

  return packages;
}

function parseCsprojFiles(dir: string): ScannedPackage[] {
  const packages: ScannedPackage[] = [];
  const seen = new Set<string>();

  let entries: string[];
  try {
    entries = fs.readdirSync(dir);
  } catch {
    return [];
  }

  const csprojFiles = entries.filter(f => f.endsWith('.csproj'));
  for (const file of csprojFiles) {
    const content = fs.readFileSync(path.join(dir, file), 'utf8');
    // <PackageReference Include="Name" Version="1.0.0" />
    // or <PackageReference Include="Name"><Version>1.0.0</Version></PackageReference>
    const selfClosing = [...content.matchAll(/<PackageReference\s+Include="([^"]+)"\s+Version="([^"]+)"\s*\/>/gi)];
    for (const [, name, version] of selfClosing) {
      if (!name || !version) continue;
      const key = `${name}@${version}`;
      if (!seen.has(key)) {
        seen.add(key);
        packages.push({ ecosystem: 'nuget', name, version });
      }
    }
    // Multi-line form: <PackageReference Include="Name">...<Version>x</Version>
    const multiLine = [...content.matchAll(/<PackageReference\s+Include="([^"]+)"[^>]*>([\s\S]*?)<\/PackageReference>/gi)];
    for (const [, name, inner] of multiLine) {
      if (!name || !inner) continue;
      const versionMatch = inner.match(/<Version>([^<]+)<\/Version>/i);
      const version = versionMatch?.[1] ?? 'unknown';
      const key = `${name}@${version}`;
      if (!seen.has(key)) {
        seen.add(key);
        packages.push({ ecosystem: 'nuget', name, version });
      }
    }
  }

  return packages;
}
