import { randomUUID } from 'node:crypto';
import fs from 'node:fs';
import yaml from 'js-yaml';
import type Database from 'better-sqlite3';
import type { LicenseCategory, Policy, PolicyResult, PolicyRules, PolicyViolation } from './types.js';
import { classifyLicense } from './license-data.js';
import type { ScanResult } from './scanner.js';

// ---------------------------------------------------------------------------
// Built-in policy templates
// ---------------------------------------------------------------------------

export interface PolicyTemplate {
  name: string;
  description: string;
  rules: PolicyRules;
}

export const BUILT_IN_TEMPLATES: Record<string, PolicyTemplate> = {
  'permissive-only': {
    name: 'permissive-only',
    description: 'Allow only permissive licenses (MIT, Apache-2.0, BSD, ISC, etc.). Denies copyleft, proprietary, and unknown licenses.',
    rules: { denyCategories: ['copyleft-weak', 'copyleft-strong', 'proprietary', 'unknown'] },
  },
  'no-copyleft-strong': {
    name: 'no-copyleft-strong',
    description: 'Deny strong copyleft licenses (GPL, AGPL, OSL). Allows weak copyleft and permissive licenses.',
    rules: { denyCategories: ['copyleft-strong'] },
  },
  'no-proprietary': {
    name: 'no-proprietary',
    description: 'Deny proprietary and source-available licenses (BUSL, SSPL, etc.).',
    rules: { denyCategories: ['proprietary'] },
  },
};

// ---------------------------------------------------------------------------
// Policy evaluation
// ---------------------------------------------------------------------------

/**
 * Evaluate a policy against the packages from a completed scan.
 * Evaluation order: explicit allow → explicit deny → category deny → pass.
 */
export function evaluatePolicy(policy: Policy, packages: ScanResult['packages']): PolicyResult {
  const violations: PolicyViolation[] = [];
  const { allow = [], deny = [], denyCategories = [] } = policy.rules;

  for (const pkg of packages) {
    const license = pkg.licenseExpression ?? null;
    const category = (pkg.licenseCategory as LicenseCategory) ?? classifyLicense(license ?? '');

    // Explicit allow overrides everything
    if (license && allow.includes(license)) continue;

    // Explicit deny by license ID
    if (license && deny.includes(license)) {
      violations.push({
        packageName: pkg.name,
        packageVersion: pkg.version,
        ecosystem: pkg.ecosystem,
        license,
        rule: `deny: ${license}`,
        remediationHint: `Replace ${pkg.name} with a package that uses an allowed license, or add an explicit allow rule.`,
      });
      continue;
    }

    // Category deny
    if (denyCategories.includes(category)) {
      violations.push({
        packageName: pkg.name,
        packageVersion: pkg.version,
        ecosystem: pkg.ecosystem,
        license,
        rule: `denyCategories: ${category}`,
        remediationHint: `${pkg.name} uses a ${category} license (${license ?? 'unknown'}). Replace it with a compatible alternative or add an explicit allow rule.`,
      });
    }
  }

  return { verdict: violations.length === 0 ? 'pass' : 'fail', violations };
}

// ---------------------------------------------------------------------------
// DB helpers — row ↔ domain object
// ---------------------------------------------------------------------------

interface PolicyRow {
  id: string;
  name: string;
  description: string | null;
  rules: string;
  created_at: string;
  updated_at: string;
}

function rowToPolicy(row: PolicyRow): Policy {
  return {
    id: row.id,
    name: row.name,
    description: row.description,
    rules: JSON.parse(row.rules) as PolicyRules,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

export function createPolicy(
  db: Database.Database,
  input: { name: string; description?: string | null; rules: PolicyRules },
): Policy {
  const id = randomUUID();
  const row = db
    .prepare(
      `INSERT INTO policies (id, name, description, rules)
       VALUES (@id, @name, @description, @rules)
       RETURNING *`,
    )
    .get({
      id,
      name: input.name,
      description: input.description ?? null,
      rules: JSON.stringify(input.rules),
    }) as PolicyRow;
  return rowToPolicy(row);
}

export function listPolicies(db: Database.Database): Policy[] {
  const rows = db
    .prepare(`SELECT * FROM policies ORDER BY created_at DESC`)
    .all() as PolicyRow[];
  return rows.map(rowToPolicy);
}

export function getPolicy(db: Database.Database, idOrName: string): Policy | null {
  const row = db
    .prepare(`SELECT * FROM policies WHERE id = ? OR name = ? LIMIT 1`)
    .get(idOrName, idOrName) as PolicyRow | undefined;
  return row ? rowToPolicy(row) : null;
}

export function updatePolicy(
  db: Database.Database,
  id: string,
  patch: { name?: string; description?: string | null; rules?: PolicyRules },
): Policy | null {
  const existing = db.prepare(`SELECT * FROM policies WHERE id = ?`).get(id) as PolicyRow | undefined;
  if (!existing) return null;

  const updated = db
    .prepare(
      `UPDATE policies
       SET name        = @name,
           description = @description,
           rules       = @rules,
           updated_at  = datetime('now')
       WHERE id = @id
       RETURNING *`,
    )
    .get({
      id,
      name: patch.name ?? existing.name,
      description: patch.description !== undefined ? patch.description : existing.description,
      rules: patch.rules !== undefined ? JSON.stringify(patch.rules) : existing.rules,
    }) as PolicyRow;
  return rowToPolicy(updated);
}

export function deletePolicy(db: Database.Database, id: string): boolean {
  const result = db.prepare(`DELETE FROM policies WHERE id = ?`).run(id);
  return result.changes > 0;
}

// ---------------------------------------------------------------------------
// YAML support
// ---------------------------------------------------------------------------

export interface PolicyFileContents {
  name?: string;
  description?: string;
  rules: PolicyRules;
}

/**
 * Parse a YAML (or JSON) string that conforms to the policy file schema.
 * Throws on invalid YAML or missing `rules` field.
 */
export function parseYamlPolicy(source: string): PolicyFileContents {
  const parsed = yaml.load(source);
  if (!parsed || typeof parsed !== 'object') {
    throw new Error('Policy file must be a YAML/JSON object');
  }
  const obj = parsed as Record<string, unknown>;
  if (!obj['rules'] || typeof obj['rules'] !== 'object') {
    throw new Error('Policy file must contain a `rules` object');
  }
  return {
    name: typeof obj['name'] === 'string' ? obj['name'] : undefined,
    description: typeof obj['description'] === 'string' ? obj['description'] : undefined,
    rules: obj['rules'] as PolicyRules,
  };
}

/**
 * Load a policy file from disk and parse it.
 * Throws if the file doesn't exist or is invalid.
 */
export function loadPolicyFile(filePath: string): PolicyFileContents {
  const source = fs.readFileSync(filePath, 'utf8');
  return parseYamlPolicy(source);
}

/**
 * Look up a policy by id/name, or create/update it from a file path.
 * Returns the policy, or null if not found and no file path given.
 */
export function resolveOrCreatePolicy(
  db: Database.Database,
  idOrName: string,
): Policy | null {
  return getPolicy(db, idOrName);
}
