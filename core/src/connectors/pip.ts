import fs from 'node:fs';
import path from 'node:path';
import type { ScannedPackage } from './types.js';

/**
 * Parse pip manifests: requirements.txt and pyproject.toml.
 */
export function scanPip(dir: string): ScannedPackage[] {
  const packages: ScannedPackage[] = [];
  const reqPath = path.join(dir, 'requirements.txt');
  if (fs.existsSync(reqPath)) {
    packages.push(...parseRequirementsTxt(reqPath));
  }
  const pyprojectPath = path.join(dir, 'pyproject.toml');
  if (fs.existsSync(pyprojectPath)) {
    packages.push(...parsePyprojectToml(pyprojectPath));
  }
  return deduplicatePip(packages);
}

function parseRequirementsTxt(filePath: string): ScannedPackage[] {
  const lines = fs.readFileSync(filePath, 'utf8').split('\n');
  const packages: ScannedPackage[] = [];
  for (let line of lines) {
    // Strip comments and whitespace
    line = line.split('#')[0]!.trim();
    if (!line || line.startsWith('-') || line.startsWith('http')) continue;
    // Handle extras like package[extra]==1.0
    const withoutExtras = line.replace(/\[.*?\]/, '');
    // Try to split on version operators
    const match = withoutExtras.match(/^([A-Za-z0-9_.\-]+)\s*([=!<>~^]+)\s*([^\s,;]+)/);
    if (match) {
      const [, name, , version] = match;
      packages.push({ ecosystem: 'pip', name: normalizePipName(name!), version: version! });
    } else {
      // No version specified
      const name = withoutExtras.match(/^([A-Za-z0-9_.\-]+)/)?.[1];
      if (name) {
        packages.push({ ecosystem: 'pip', name: normalizePipName(name), version: 'unknown' });
      }
    }
  }
  return packages;
}

/** Naive TOML parser for pyproject.toml — extracts dependencies arrays. */
function parsePyprojectToml(filePath: string): ScannedPackage[] {
  const content = fs.readFileSync(filePath, 'utf8');
  const packages: ScannedPackage[] = [];
  // Match [project] dependencies = [...] or [tool.poetry.dependencies]
  const depMatches = [
    // PEP 621 inline array
    /\[project\][^\[]*dependencies\s*=\s*\[([\s\S]*?)\]/,
    // Poetry style
    /\[tool\.poetry\.dependencies\]([\s\S]*?)(?=\[|$)/,
  ];
  for (const re of depMatches) {
    const m = content.match(re);
    if (!m) continue;
    const block = m[1]!;
    // Extract quoted strings for PEP 621 style
    const quotedDeps = [...block.matchAll(/"([^"]+)"|'([^']+)'/g)].map(x => x[1] ?? x[2] ?? '');
    for (const dep of quotedDeps) {
      const match = dep.match(/^([A-Za-z0-9_.\-]+)\s*([=!<>~^]+)?\s*([^\s,;]*)/);
      if (match) {
        packages.push({
          ecosystem: 'pip',
          name: normalizePipName(match[1]!),
          version: match[3] ? match[3].replace(/^[=^~]+/, '') : 'unknown',
        });
      }
    }
    // For Poetry style: key = "version"
    const poetryDeps = [...block.matchAll(/^([A-Za-z0-9_.\-]+)\s*=\s*"([^"]+)"/gm)];
    for (const [, name, version] of poetryDeps) {
      if (name === 'python') continue;
      packages.push({
        ecosystem: 'pip',
        name: normalizePipName(name!),
        version: version!.replace(/^[^0-9]*/, '') || version!,
      });
    }
  }
  return packages;
}

/** PEP 503 normalization: lowercase, replace [-_.] with - */
function normalizePipName(name: string): string {
  return name.toLowerCase().replace(/[-_.]+/g, '-');
}

function deduplicatePip(packages: ScannedPackage[]): ScannedPackage[] {
  const seen = new Set<string>();
  return packages.filter(p => {
    const key = `${p.name}@${p.version}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
