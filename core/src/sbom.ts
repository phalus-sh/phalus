import fs from 'node:fs';
import path from 'node:path';
import type { Ecosystem } from './types.js';
import type { ScannedPackage } from './connectors/types.js';

// ── CycloneDX JSON ────────────────────────────────────────────────────────────

interface CdxLicense {
  license?: { id?: string; name?: string; text?: { content?: string } };
  expression?: string;
}

interface CdxComponent {
  type?: string;
  name?: string;
  version?: string;
  purl?: string;
  licenses?: CdxLicense[];
}

interface CdxBom {
  bomFormat?: string;
  components?: CdxComponent[];
}

export function parseCycloneDX(filePath: string): ScannedPackage[] {
  const raw = JSON.parse(fs.readFileSync(filePath, 'utf8')) as CdxBom;
  if (raw.bomFormat !== 'CycloneDX') {
    throw new Error(`Not a CycloneDX BOM: ${filePath}`);
  }
  const packages: ScannedPackage[] = [];
  for (const comp of raw.components ?? []) {
    const name = comp.name ?? 'unknown';
    const version = comp.version ?? 'unknown';
    const ecosystem = purlToEcosystem(comp.purl);
    const licenseExpression = extractCdxLicense(comp.licenses ?? []);
    packages.push({
      ecosystem,
      name,
      version,
      licenseExpression: licenseExpression || undefined,
      licenseSource: licenseExpression ? 'cyclonedx-sbom' : undefined,
    });
  }
  return packages;
}

function extractCdxLicense(licenses: CdxLicense[]): string {
  const parts: string[] = [];
  for (const l of licenses) {
    if (l.expression) {
      parts.push(l.expression);
    } else if (l.license?.id) {
      parts.push(l.license.id);
    } else if (l.license?.name) {
      parts.push(l.license.name);
    }
  }
  return parts.join(' AND ');
}

// ── SPDX JSON ─────────────────────────────────────────────────────────────────

interface SpdxPackage {
  name?: string;
  versionInfo?: string;
  licenseConcluded?: string;
  licenseDeclared?: string;
  externalRefs?: Array<{ referenceCategory?: string; referenceType?: string; referenceLocator?: string }>;
}

interface SpdxDoc {
  spdxVersion?: string;
  packages?: SpdxPackage[];
}

export function parseSpdx(filePath: string): ScannedPackage[] {
  const raw = JSON.parse(fs.readFileSync(filePath, 'utf8')) as SpdxDoc;
  if (!raw.spdxVersion?.startsWith('SPDX-')) {
    throw new Error(`Not an SPDX document: ${filePath}`);
  }
  const packages: ScannedPackage[] = [];
  for (const pkg of raw.packages ?? []) {
    const name = pkg.name ?? 'unknown';
    const version = pkg.versionInfo ?? 'unknown';
    const licenseExpression =
      pkg.licenseConcluded && pkg.licenseConcluded !== 'NOASSERTION'
        ? pkg.licenseConcluded
        : pkg.licenseDeclared && pkg.licenseDeclared !== 'NOASSERTION'
        ? pkg.licenseDeclared
        : undefined;
    // Try to derive ecosystem from purl externalRef
    const purl = pkg.externalRefs?.find(r => r.referenceType === 'purl')?.referenceLocator;
    const ecosystem = purlToEcosystem(purl);
    packages.push({
      ecosystem,
      name,
      version,
      licenseExpression,
      licenseSource: licenseExpression ? 'spdx-sbom' : undefined,
    });
  }
  return packages;
}

// ── Auto-detect SBOM files in a directory ────────────────────────────────────

export function scanSboms(dir: string): ScannedPackage[] {
  const packages: ScannedPackage[] = [];
  const entries = fs.readdirSync(dir);
  for (const entry of entries) {
    if (!entry.endsWith('.json')) continue;
    const filePath = path.join(dir, entry);
    try {
      const raw = JSON.parse(fs.readFileSync(filePath, 'utf8')) as Record<string, unknown>;
      if (raw['bomFormat'] === 'CycloneDX') {
        packages.push(...parseCycloneDX(filePath));
      } else if (typeof raw['spdxVersion'] === 'string' && raw['spdxVersion'].startsWith('SPDX-')) {
        packages.push(...parseSpdx(filePath));
      }
    } catch {
      // Not a valid JSON or SBOM — skip
    }
  }
  return packages;
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function purlToEcosystem(purl?: string): Ecosystem {
  if (!purl) return 'unknown';
  if (purl.startsWith('pkg:npm')) return 'npm';
  if (purl.startsWith('pkg:pypi')) return 'pip';
  if (purl.startsWith('pkg:cargo')) return 'cargo';
  if (purl.startsWith('pkg:golang')) return 'go';
  if (purl.startsWith('pkg:maven')) return 'maven';
  if (purl.startsWith('pkg:nuget')) return 'nuget';
  return 'unknown';
}
