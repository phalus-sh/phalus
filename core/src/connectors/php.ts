import fs from 'node:fs';
import path from 'node:path';
import type { ScannedPackage } from './types.js';

/**
 * Parse PHP/Composer manifests.
 * Reads composer.lock — license info is self-contained in the lock file.
 */
export function scanPhp(dir: string): ScannedPackage[] {
  const lockPath = path.join(dir, 'composer.lock');
  if (!fs.existsSync(lockPath)) return [];
  return parseComposerLock(lockPath);
}

function parseComposerLock(lockPath: string): ScannedPackage[] {
  const raw = JSON.parse(fs.readFileSync(lockPath, 'utf8')) as Record<string, unknown>;
  const packages: ScannedPackage[] = [];

  for (const section of ['packages', 'packages-dev'] as const) {
    const pkgs = raw[section];
    if (!Array.isArray(pkgs)) continue;
    for (const pkg of pkgs as Record<string, unknown>[]) {
      const name = typeof pkg['name'] === 'string' ? pkg['name'] : undefined;
      const version = typeof pkg['version'] === 'string' ? normalizeVersion(pkg['version']) : 'unknown';
      if (!name) continue;

      // license field is an array of SPDX identifiers in composer.lock
      let licenseExpression: string | undefined;
      let licenseSource: string | undefined;
      const licenseField = pkg['license'];
      if (Array.isArray(licenseField) && licenseField.length > 0) {
        licenseExpression = (licenseField as string[]).join(' OR ');
        licenseSource = 'composer.lock';
      } else if (typeof licenseField === 'string' && licenseField) {
        licenseExpression = licenseField;
        licenseSource = 'composer.lock';
      }

      packages.push({ ecosystem: 'php', name, version, licenseExpression, licenseSource });
    }
  }

  return packages;
}

/** Strip "v" prefix from composer versions (e.g. "v1.2.3" → "1.2.3"). */
function normalizeVersion(v: string): string {
  return v.replace(/^v/, '');
}
