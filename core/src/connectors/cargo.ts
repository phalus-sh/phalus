import fs from 'node:fs';
import path from 'node:path';
import type { ScannedPackage } from './types.js';

/**
 * Parse Cargo manifests.
 * Prefers Cargo.lock (authoritative versions) over Cargo.toml.
 */
export function scanCargo(dir: string): ScannedPackage[] {
  const lockPath = path.join(dir, 'Cargo.lock');
  if (fs.existsSync(lockPath)) {
    return parseCargoLock(lockPath);
  }
  const tomlPath = path.join(dir, 'Cargo.toml');
  if (fs.existsSync(tomlPath)) {
    return parseCargoToml(tomlPath);
  }
  return [];
}

/** Parse Cargo.lock — TOML with [[package]] sections. */
function parseCargoLock(lockPath: string): ScannedPackage[] {
  const content = fs.readFileSync(lockPath, 'utf8');
  const packages: ScannedPackage[] = [];
  // Split on [[package]] boundaries
  const blocks = content.split(/^\[\[package\]\]/m).slice(1);
  for (const block of blocks) {
    const name = extractTomlString(block, 'name');
    const version = extractTomlString(block, 'version');
    if (name && version) {
      packages.push({ ecosystem: 'cargo', name, version });
    }
  }
  return packages;
}

/** Parse Cargo.toml — extract [dependencies] and [dev-dependencies]. */
function parseCargoToml(tomlPath: string): ScannedPackage[] {
  const content = fs.readFileSync(tomlPath, 'utf8');
  const packages: ScannedPackage[] = [];
  const sections = ['dependencies', 'dev-dependencies', 'build-dependencies'];
  for (const section of sections) {
    const re = new RegExp(`\\[${section}\\]([\\s\\S]*?)(?=\\[|$)`);
    const m = content.match(re);
    if (!m) continue;
    const block = m[1]!;
    // Simple "name = version" lines
    const simpleLines = [...block.matchAll(/^([a-zA-Z0-9_\-]+)\s*=\s*"([^"]+)"/gm)];
    for (const [, name, version] of simpleLines) {
      packages.push({ ecosystem: 'cargo', name: name!, version: version!.replace(/^[^0-9]*/, '') || version! });
    }
    // Inline table: name = { version = "..." }
    const inlineLines = [...block.matchAll(/^([a-zA-Z0-9_\-]+)\s*=\s*\{[^}]*version\s*=\s*"([^"]+)"/gm)];
    for (const [, name, version] of inlineLines) {
      packages.push({ ecosystem: 'cargo', name: name!, version: version!.replace(/^[^0-9]*/, '') || version! });
    }
  }
  return packages;
}

function extractTomlString(block: string, key: string): string | undefined {
  const m = block.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, 'm'));
  return m?.[1];
}
