import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import Database from 'better-sqlite3';
import { CREATE_TABLES_SQL } from './schema.js';
import {
  evaluatePolicy,
  createPolicy,
  listPolicies,
  getPolicy,
  updatePolicy,
  deletePolicy,
  BUILT_IN_TEMPLATES,
  parseYamlPolicy,
} from './policy.js';
import type { Policy } from './types.js';

let db: Database.Database;

beforeEach(() => {
  db = new Database(':memory:');
  db.pragma('foreign_keys = ON');
  db.exec(CREATE_TABLES_SQL);
});

afterEach(() => {
  db.close();
});

// ---------------------------------------------------------------------------
// evaluatePolicy
// ---------------------------------------------------------------------------

describe('evaluatePolicy', () => {
  const base: Omit<Policy, 'id' | 'createdAt' | 'updatedAt'> = {
    name: 'test',
    description: null,
    rules: {},
  };

  function pkg(
    name: string,
    licenseExpression: string | null,
    licenseCategory: string,
    ecosystem = 'npm',
  ) {
    return { name, version: '1.0.0', ecosystem, licenseExpression, licenseCategory };
  }

  it('passes when no packages', () => {
    const policy: Policy = { ...base, id: 'p1', createdAt: '', updatedAt: '', rules: {} };
    const result = evaluatePolicy(policy, []);
    expect(result.verdict).toBe('pass');
    expect(result.violations).toHaveLength(0);
  });

  it('passes permissive packages against permissive-only policy', () => {
    const policy: Policy = {
      ...base,
      id: 'p1', createdAt: '', updatedAt: '',
      rules: { denyCategories: ['copyleft-weak', 'copyleft-strong', 'proprietary', 'unknown'] },
    };
    const result = evaluatePolicy(policy, [
      pkg('lodash', 'MIT', 'permissive'),
      pkg('axios', 'Apache-2.0', 'permissive'),
    ]);
    expect(result.verdict).toBe('pass');
    expect(result.violations).toHaveLength(0);
  });

  it('fails when copyleft-strong license violates denyCategories', () => {
    const policy: Policy = {
      ...base,
      id: 'p1', createdAt: '', updatedAt: '',
      rules: { denyCategories: ['copyleft-strong'] },
    };
    const result = evaluatePolicy(policy, [
      pkg('gpl-lib', 'GPL-3.0-only', 'copyleft-strong'),
    ]);
    expect(result.verdict).toBe('fail');
    expect(result.violations).toHaveLength(1);
    expect(result.violations[0]!.packageName).toBe('gpl-lib');
    expect(result.violations[0]!.rule).toContain('denyCategories');
  });

  it('explicit allow overrides denyCategories', () => {
    const policy: Policy = {
      ...base,
      id: 'p1', createdAt: '', updatedAt: '',
      rules: { denyCategories: ['copyleft-strong'], allow: ['GPL-3.0-only'] },
    };
    const result = evaluatePolicy(policy, [
      pkg('gpl-lib', 'GPL-3.0-only', 'copyleft-strong'),
    ]);
    expect(result.verdict).toBe('pass');
  });

  it('explicit deny triggers violation regardless of category', () => {
    const policy: Policy = {
      ...base,
      id: 'p1', createdAt: '', updatedAt: '',
      rules: { deny: ['MIT'] },
    };
    const result = evaluatePolicy(policy, [
      pkg('lodash', 'MIT', 'permissive'),
    ]);
    expect(result.verdict).toBe('fail');
    expect(result.violations[0]!.rule).toBe('deny: MIT');
  });

  it('allow overrides explicit deny', () => {
    const policy: Policy = {
      ...base,
      id: 'p1', createdAt: '', updatedAt: '',
      rules: { deny: ['MIT'], allow: ['MIT'] },
    };
    const result = evaluatePolicy(policy, [pkg('lodash', 'MIT', 'permissive')]);
    expect(result.verdict).toBe('pass');
  });

  it('packages with null license are evaluated as unknown category', () => {
    const policy: Policy = {
      ...base,
      id: 'p1', createdAt: '', updatedAt: '',
      rules: { denyCategories: ['unknown'] },
    };
    const result = evaluatePolicy(policy, [
      pkg('mystery', null, 'unknown'),
    ]);
    expect(result.verdict).toBe('fail');
  });

  it('violation includes remediation hint', () => {
    const policy: Policy = {
      ...base,
      id: 'p1', createdAt: '', updatedAt: '',
      rules: { denyCategories: ['proprietary'] },
    };
    const result = evaluatePolicy(policy, [
      pkg('closed-lib', 'BUSL-1.1', 'proprietary'),
    ]);
    expect(result.violations[0]!.remediationHint).toBeTruthy();
  });
});

// ---------------------------------------------------------------------------
// Built-in templates
// ---------------------------------------------------------------------------

describe('BUILT_IN_TEMPLATES', () => {
  it('has permissive-only, no-copyleft-strong, no-proprietary', () => {
    expect(BUILT_IN_TEMPLATES['permissive-only']).toBeDefined();
    expect(BUILT_IN_TEMPLATES['no-copyleft-strong']).toBeDefined();
    expect(BUILT_IN_TEMPLATES['no-proprietary']).toBeDefined();
  });

  it('permissive-only denies copyleft-strong packages', () => {
    const t = BUILT_IN_TEMPLATES['permissive-only']!;
    const policy: Policy = { id: 't', name: t.name, description: t.description, rules: t.rules, createdAt: '', updatedAt: '' };
    const result = evaluatePolicy(policy, [
      { name: 'gpl-lib', version: '1.0.0', ecosystem: 'npm', licenseExpression: 'GPL-3.0-only', licenseCategory: 'copyleft-strong' },
    ]);
    expect(result.verdict).toBe('fail');
  });

  it('no-proprietary allows GPL packages', () => {
    const t = BUILT_IN_TEMPLATES['no-proprietary']!;
    const policy: Policy = { id: 't', name: t.name, description: t.description, rules: t.rules, createdAt: '', updatedAt: '' };
    const result = evaluatePolicy(policy, [
      { name: 'gpl-lib', version: '1.0.0', ecosystem: 'npm', licenseExpression: 'GPL-3.0-only', licenseCategory: 'copyleft-strong' },
    ]);
    expect(result.verdict).toBe('pass');
  });
});

// ---------------------------------------------------------------------------
// CRUD
// ---------------------------------------------------------------------------

describe('policy CRUD', () => {
  it('createPolicy and getPolicy round-trip', () => {
    const p = createPolicy(db, { name: 'my-policy', description: 'desc', rules: { denyCategories: ['proprietary'] } });
    expect(p.id).toBeTruthy();
    expect(p.name).toBe('my-policy');
    const fetched = getPolicy(db, p.id);
    expect(fetched).not.toBeNull();
    expect(fetched!.rules.denyCategories).toContain('proprietary');
  });

  it('getPolicy by name', () => {
    createPolicy(db, { name: 'named-policy', rules: { deny: ['MIT'] } });
    const p = getPolicy(db, 'named-policy');
    expect(p).not.toBeNull();
    expect(p!.name).toBe('named-policy');
  });

  it('listPolicies returns all policies', () => {
    createPolicy(db, { name: 'p1', rules: {} });
    createPolicy(db, { name: 'p2', rules: {} });
    const list = listPolicies(db);
    expect(list.length).toBe(2);
  });

  it('updatePolicy changes fields', () => {
    const p = createPolicy(db, { name: 'old-name', rules: {} });
    const updated = updatePolicy(db, p.id, { name: 'new-name', rules: { deny: ['MIT'] } });
    expect(updated).not.toBeNull();
    expect(updated!.name).toBe('new-name');
    expect(updated!.rules.deny).toContain('MIT');
  });

  it('updatePolicy returns null for missing id', () => {
    const result = updatePolicy(db, 'nonexistent', { name: 'x' });
    expect(result).toBeNull();
  });

  it('deletePolicy removes the policy', () => {
    const p = createPolicy(db, { name: 'to-delete', rules: {} });
    expect(deletePolicy(db, p.id)).toBe(true);
    expect(getPolicy(db, p.id)).toBeNull();
  });

  it('deletePolicy returns false for missing id', () => {
    expect(deletePolicy(db, 'nonexistent')).toBe(false);
  });

  it('rejects duplicate policy names', () => {
    createPolicy(db, { name: 'dup', rules: {} });
    expect(() => createPolicy(db, { name: 'dup', rules: {} })).toThrow();
  });
});

// ---------------------------------------------------------------------------
// parseYamlPolicy
// ---------------------------------------------------------------------------

describe('parseYamlPolicy', () => {
  it('parses minimal rules object', () => {
    const result = parseYamlPolicy(`rules:\n  denyCategories:\n    - copyleft-strong\n`);
    expect(result.rules.denyCategories).toContain('copyleft-strong');
  });

  it('parses name and description', () => {
    const result = parseYamlPolicy(
      `name: my-policy\ndescription: A test\nrules:\n  deny:\n    - MIT\n`,
    );
    expect(result.name).toBe('my-policy');
    expect(result.description).toBe('A test');
    expect(result.rules.deny).toContain('MIT');
  });

  it('throws when rules is missing', () => {
    expect(() => parseYamlPolicy('name: oops\n')).toThrow();
  });

  it('also parses JSON (YAML superset)', () => {
    const json = JSON.stringify({ rules: { allow: ['MIT'] } });
    const result = parseYamlPolicy(json);
    expect(result.rules.allow).toContain('MIT');
  });
});
