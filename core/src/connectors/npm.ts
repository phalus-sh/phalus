import fs from 'node:fs';
import path from 'node:path';
import type { ScannedPackage } from './types.js';

/** Strip semver range operators and return a clean version string. */
function cleanVersion(v: string): string {
  return v.replace(/^[\^~>=<]+/, '').split(' ')[0] ?? v;
}

/**
 * Parse npm manifests in a directory.
 * Uses package-lock.json (v2/v3) when present for resolved versions,
 * otherwise falls back to package.json dependency declarations.
 */
export function scanNpm(dir: string): ScannedPackage[] {
  const lockPath = path.join(dir, 'package-lock.json');
  if (fs.existsSync(lockPath)) {
    return parsePackageLock(lockPath);
  }
  const pkgPath = path.join(dir, 'package.json');
  if (fs.existsSync(pkgPath)) {
    return parsePackageJson(pkgPath);
  }
  return [];
}

function parsePackageLock(lockPath: string): ScannedPackage[] {
  const raw = JSON.parse(fs.readFileSync(lockPath, 'utf8')) as Record<string, unknown>;
  const packages: ScannedPackage[] = [];
  // lockfileVersion 2/3 uses "packages" map
  if (raw['packages'] && typeof raw['packages'] === 'object') {
    const pkgs = raw['packages'] as Record<string, Record<string, unknown>>;
    for (const [key, meta] of Object.entries(pkgs)) {
      if (!key || key === '') continue; // root entry
      // key format: "node_modules/foo" or "node_modules/foo/node_modules/bar"
      const name = key.replace(/^.*node_modules\//, '');
      const version = typeof meta['version'] === 'string' ? meta['version'] : 'unknown';
      const license = typeof meta['license'] === 'string' ? meta['license'] : undefined;
      packages.push({
        ecosystem: 'npm',
        name,
        version,
        licenseExpression: license,
        licenseSource: license ? 'package-lock.json' : undefined,
      });
    }
    return packages;
  }
  // lockfileVersion 1 uses "dependencies" map
  if (raw['dependencies'] && typeof raw['dependencies'] === 'object') {
    return flattenLockV1(raw['dependencies'] as Record<string, Record<string, unknown>>);
  }
  return [];
}

function flattenLockV1(deps: Record<string, Record<string, unknown>>): ScannedPackage[] {
  const packages: ScannedPackage[] = [];
  for (const [name, meta] of Object.entries(deps)) {
    const version = typeof meta['version'] === 'string' ? meta['version'] : 'unknown';
    packages.push({ ecosystem: 'npm', name, version });
    if (meta['dependencies'] && typeof meta['dependencies'] === 'object') {
      packages.push(...flattenLockV1(meta['dependencies'] as Record<string, Record<string, unknown>>));
    }
  }
  return packages;
}

function parsePackageJson(pkgPath: string): ScannedPackage[] {
  const raw = JSON.parse(fs.readFileSync(pkgPath, 'utf8')) as Record<string, unknown>;
  const packages: ScannedPackage[] = [];
  const depSections = ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies'] as const;
  for (const section of depSections) {
    const deps = raw[section];
    if (deps && typeof deps === 'object') {
      for (const [name, version] of Object.entries(deps as Record<string, string>)) {
        packages.push({
          ecosystem: 'npm',
          name,
          version: cleanVersion(version),
          licenseSource: 'package.json',
        });
      }
    }
  }
  return packages;
}
