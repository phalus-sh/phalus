import fs from 'node:fs';
import path from 'node:path';
import type { ScannedPackage } from './types.js';

/**
 * Parse go.mod manifest.
 */
export function scanGo(dir: string): ScannedPackage[] {
  const modPath = path.join(dir, 'go.mod');
  if (!fs.existsSync(modPath)) return [];
  return parseGoMod(modPath);
}

function parseGoMod(modPath: string): ScannedPackage[] {
  const content = fs.readFileSync(modPath, 'utf8');
  const packages: ScannedPackage[] = [];
  // Match require blocks: require ( ... ) or single require statements
  // require block
  const blockRe = /require\s*\(([\s\S]*?)\)/g;
  let m: RegExpExecArray | null;
  while ((m = blockRe.exec(content)) !== null) {
    const block = m[1]!;
    packages.push(...parseRequireBlock(block));
  }
  // Single-line require
  const singleRe = /^require\s+([^\s(]+)\s+([^\s]+)/gm;
  while ((m = singleRe.exec(content)) !== null) {
    packages.push({ ecosystem: 'go', name: m[1]!, version: stripGoVersionSuffix(m[2]!) });
  }
  // Deduplicate
  const seen = new Set<string>();
  return packages.filter(p => {
    const key = `${p.name}@${p.version}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

function parseRequireBlock(block: string): ScannedPackage[] {
  const packages: ScannedPackage[] = [];
  for (const line of block.split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('//')) continue;
    // Format: module/path v1.2.3 [// indirect]
    const parts = trimmed.split(/\s+/);
    if (parts.length >= 2) {
      const name = parts[0]!;
      const version = stripGoVersionSuffix(parts[1]!);
      packages.push({ ecosystem: 'go', name, version });
    }
  }
  return packages;
}

/** Remove "+incompatible" suffix from go module versions. */
function stripGoVersionSuffix(v: string): string {
  return v.replace(/\+.*$/, '');
}
