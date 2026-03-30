import fs from 'node:fs';
import path from 'node:path';
import type { ScannedPackage } from './types.js';

/**
 * Parse Maven manifests.
 * Reads pom.xml and extracts <dependency> elements from both
 * <dependencies> and <dependencyManagement> sections.
 */
export function scanMaven(dir: string): ScannedPackage[] {
  const pomPath = path.join(dir, 'pom.xml');
  if (!fs.existsSync(pomPath)) return [];
  return parsePom(pomPath);
}

function parsePom(pomPath: string): ScannedPackage[] {
  const content = fs.readFileSync(pomPath, 'utf8');
  const packages: ScannedPackage[] = [];

  // Extract all <dependency> blocks
  const depBlocks = [...content.matchAll(/<dependency>([\s\S]*?)<\/dependency>/g)];
  for (const [, block] of depBlocks) {
    if (!block) continue;
    const groupId = extractXmlTag(block, 'groupId');
    const artifactId = extractXmlTag(block, 'artifactId');
    const version = extractXmlTag(block, 'version');
    if (!groupId || !artifactId) continue;
    // Skip test-scoped deps
    const scope = extractXmlTag(block, 'scope');
    if (scope === 'test') continue;
    packages.push({
      ecosystem: 'maven',
      name: `${groupId}:${artifactId}`,
      version: version ?? 'unknown',
    });
  }

  // Extract project-level license if declared
  const licenseExpression = extractProjectLicense(content);

  // Deduplicate by name+version
  const seen = new Set<string>();
  const deduped: ScannedPackage[] = [];
  for (const pkg of packages) {
    const key = `${pkg.name}@${pkg.version}`;
    if (!seen.has(key)) {
      seen.add(key);
      // Apply project-level license as a hint if present (dependencies typically
      // don't carry their own license in pom.xml; the project license is for the
      // project itself, not its transitive deps)
      deduped.push(licenseExpression ? { ...pkg, licenseExpression, licenseSource: 'pom.xml' } : pkg);
    }
  }
  return deduped;
}

function extractXmlTag(content: string, tag: string): string | undefined {
  const m = content.match(new RegExp(`<${tag}>([^<]+)<\\/${tag}>`));
  return m?.[1]?.trim();
}

function extractProjectLicense(content: string): string | undefined {
  // <licenses><license><name>MIT</name></license></licenses>
  const licensesBlock = content.match(/<licenses>([\s\S]*?)<\/licenses>/)?.[1];
  if (!licensesBlock) return undefined;
  const name = extractXmlTag(licensesBlock, 'name');
  return name || undefined;
}
