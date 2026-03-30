import fs from 'node:fs';
import path from 'node:path';
import type { ScannedPackage } from './types.js';

/**
 * Parse Ruby manifests.
 * Reads Gemfile.lock and extracts gem dependencies with versions.
 * Path/git gems are included with version 'unknown'.
 */
export function scanRuby(dir: string): ScannedPackage[] {
  const lockPath = path.join(dir, 'Gemfile.lock');
  if (!fs.existsSync(lockPath)) return [];
  return parseGemfileLock(lockPath);
}

function parseGemfileLock(lockPath: string): ScannedPackage[] {
  const content = fs.readFileSync(lockPath, 'utf8');
  const packages: ScannedPackage[] = [];
  const seen = new Set<string>();

  // GEM section: specs block contains "    gemname (version)" lines
  const gemSection = content.match(/^GEM\b([\s\S]*?)(?=^\S|\Z)/m)?.[1] ?? '';
  const specsBlock = gemSection.match(/^\s+specs:([\s\S]*)/m)?.[1] ?? '';
  // Top-level gems have 4-space indent: "    name (version)"
  // Sub-dependencies have 6+ spaces: skip them to avoid duplicating
  const specLines = [...specsBlock.matchAll(/^    ([a-zA-Z0-9_\-.]+)\s+\(([^)]+)\)/gm)];
  for (const [, name, version] of specLines) {
    if (!name || !version) continue;
    const key = `${name}@${version}`;
    if (!seen.has(key)) {
      seen.add(key);
      packages.push({ ecosystem: 'ruby', name, version });
    }
  }

  // PATH section: local path gems — version extracted from spec line
  const pathSections = [...content.matchAll(/^PATH\b([\s\S]*?)(?=^\S|\Z)/gm)];
  for (const [, section] of pathSections) {
    const pathSpecs = [...(section ?? '').matchAll(/^    ([a-zA-Z0-9_\-.]+)\s+\(([^)]+)\)/gm)];
    for (const [, name, version] of pathSpecs) {
      if (!name || !version) continue;
      const key = `${name}@${version}`;
      if (!seen.has(key)) {
        seen.add(key);
        packages.push({ ecosystem: 'ruby', name, version });
      }
    }
  }

  // GIT section: git-sourced gems — version from spec line
  const gitSections = [...content.matchAll(/^GIT\b([\s\S]*?)(?=^\S|\Z)/gm)];
  for (const [, section] of gitSections) {
    const gitSpecs = [...(section ?? '').matchAll(/^    ([a-zA-Z0-9_\-.]+)\s+\(([^)]+)\)/gm)];
    for (const [, name, version] of gitSpecs) {
      if (!name || !version) continue;
      const key = `${name}@${version}`;
      if (!seen.has(key)) {
        seen.add(key);
        packages.push({ ecosystem: 'ruby', name, version });
      }
    }
  }

  return packages;
}
