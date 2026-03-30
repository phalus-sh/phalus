#!/usr/bin/env node
import fs from 'node:fs';
import path from 'node:path';
import {
  initDb,
  createScanRun,
  runScan,
  classifyLicense,
  listPolicies,
  getPolicy,
  createPolicy,
  updatePolicy,
  loadPolicyFile,
  BUILT_IN_TEMPLATES,
} from '@phalus/core';
import { loadConfig } from './config.js';


const [, , command, ...args] = process.argv;

function help(): void {
  console.log(`
phalus <command> [options]

Commands:
  scan <path>            Scan a project directory for license data
  policy list            List all stored policies and built-in templates
  policy show <name>     Show a policy by name or id (built-in templates supported)
  help                   Show this help

Options (scan):
  --policy <name>          Policy name or ID to enforce (e.g. permissive-only)
  --fail-on-violation      Exit 1 if policy violations are found (default when --policy set)
  --no-fail-on-violation   Never exit 1 due to policy violations
  --db <path>              SQLite database path (default: ./phalus.db or PHALUS_DB_PATH)
  --json                   Output results as JSON

Exit codes:
  0  Clean scan or policy passed
  1  Policy violations found
  2  Scan error or fatal

Examples:
  phalus scan ./my-project
  phalus scan . --policy permissive-only
  phalus scan . --policy no-copyleft-strong --fail-on-violation
  phalus scan /path/to/repo --json
  phalus policy list
  phalus policy show permissive-only
`);
}

// ---------------------------------------------------------------------------
// Policy resolution helpers
// ---------------------------------------------------------------------------

function resolvePolicyId(db: ReturnType<typeof initDb>, nameOrId: string): string | null {
  const existing = getPolicy(db, nameOrId);
  if (existing) return existing.id;
  const template = BUILT_IN_TEMPLATES[nameOrId];
  if (template) {
    const created = createPolicy(db, {
      name: template.name,
      description: template.description,
      rules: template.rules,
    });
    return created.id;
  }
  return null;
}

function autoDetectPolicyFile(
  db: ReturnType<typeof initDb>,
  projectRoot: string,
  quiet: boolean,
): string | null {
  const candidates = ['.phalus-policy.yml', '.phalus-policy.yaml'];
  for (const name of candidates) {
    const filePath = path.join(projectRoot, name);
    if (fs.existsSync(filePath)) {
      if (!quiet) {
        console.log(`Auto-detected policy file: ${path.relative(process.cwd(), filePath)}`);
      }
      const contents = loadPolicyFile(filePath);
      const policyName = contents.name ?? path.basename(filePath, path.extname(filePath));
      const existing = getPolicy(db, policyName);
      if (existing) {
        updatePolicy(db, existing.id, { description: contents.description, rules: contents.rules });
        return existing.id;
      }
      const created = createPolicy(db, {
        name: policyName,
        description: contents.description ?? null,
        rules: contents.rules,
      });
      return created.id;
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Flags parser
// ---------------------------------------------------------------------------

interface ScanFlags {
  json: boolean;
  db?: string;
  policy?: string;
  failOnViolation?: boolean;
}

function parseFlags(rawArgs: string[]): { positional: string[] } & ScanFlags {
  const positional: string[] = [];
  let json = false;
  let db: string | undefined;
  let policy: string | undefined;
  let failOnViolation: boolean | undefined;

  for (let i = 0; i < rawArgs.length; i++) {
    const arg = rawArgs[i]!;
    if (arg === '--json') {
      json = true;
    } else if (arg === '--db' && rawArgs[i + 1]) {
      db = rawArgs[++i];
    } else if (arg === '--policy' && rawArgs[i + 1]) {
      policy = rawArgs[++i];
    } else if (arg === '--fail-on-violation') {
      failOnViolation = true;
    } else if (arg === '--no-fail-on-violation') {
      failOnViolation = false;
    } else {
      positional.push(arg);
    }
  }

  return { positional, json, db, policy, failOnViolation };
}

// ---------------------------------------------------------------------------
// scan command
// ---------------------------------------------------------------------------

async function cmdScan(targetPath: string, flags: ScanFlags): Promise<void> {
  const resolvedPath = path.resolve(targetPath);

  const fileConfig = loadConfig(resolvedPath);
  const policyArg = flags.policy ?? fileConfig.policy;
  const failOnViolation =
    flags.failOnViolation !== undefined
      ? flags.failOnViolation
      : (fileConfig.failOnViolation ?? (policyArg !== undefined ? true : false));

  const db = initDb(flags.db);

  let policyId: string | null = null;
  if (policyArg) {
    policyId = resolvePolicyId(db, policyArg);
    if (!policyId) {
      console.error(
        `Error: policy "${policyArg}" not found. Use "phalus policy list" to see available policies.`,
      );
      process.exit(2);
    }
  } else {
    policyId = autoDetectPolicyFile(db, resolvedPath, flags.json);
  }

  if (!flags.json) {
    console.log(`Scanning ${resolvedPath} ...`);
  }

  const scanRunId = createScanRun(db, resolvedPath);
  const result = await runScan(db, scanRunId, resolvedPath, policyId ?? undefined);

  if (result.error) {
    if (flags.json) {
      process.stdout.write(
        JSON.stringify({ error: result.error, scanRunId: result.scanRunId }, null, 2) + '\n',
      );
    } else {
      console.error(`\nError: ${result.error}`);
    }
    process.exit(2);
  }

  const hasViolations = result.policyVerdict === 'fail';

  if (flags.json) {
    process.stdout.write(JSON.stringify(result, null, 2) + '\n');
  } else {
    const { packages, alerts } = result;
    console.log(`\nFound ${packages.length} package(s)\n`);

    const byEco: Record<string, typeof packages> = {};
    for (const pkg of packages) {
      (byEco[pkg.ecosystem] ??= []).push(pkg);
    }
    for (const [eco, pkgs] of Object.entries(byEco)) {
      console.log(`  ${eco} (${pkgs.length})`);
      for (const pkg of pkgs.slice(0, 10)) {
        const lic = pkg.licenseExpression ?? 'unknown';
        const cat = classifyLicense(lic);
        console.log(`    ${pkg.name}@${pkg.version}  ${lic}  [${cat}]`);
      }
      if (pkgs.length > 10) {
        console.log(`    ... and ${pkgs.length - 10} more`);
      }
    }

    if (alerts.length > 0) {
      console.log(`\n${alerts.length} alert(s):`);
      for (const alert of alerts) {
        const icon = alert.severity === 'high' || alert.severity === 'critical' ? '✗' : '⚠';
        console.log(`  ${icon} [${alert.severity}] ${alert.message}`);
      }
    } else {
      console.log('\nNo alerts.');
    }

    if (result.policyVerdict !== undefined && result.policyVerdict !== null) {
      const verdict = result.policyVerdict;
      console.log(`\nPolicy verdict: ${verdict === 'pass' ? '✓' : '✗'} ${verdict.toUpperCase()}`);
      if (verdict === 'fail' && result.policyViolations && result.policyViolations.length > 0) {
        process.stderr.write(`\n${result.policyViolations.length} policy violation(s):\n`);
        for (const v of result.policyViolations) {
          process.stderr.write(
            `  ✗ ${v.ecosystem}:${v.packageName}@${v.packageVersion}  [${v.rule}]\n    ${v.remediationHint}\n`,
          );
        }
      }
    }

    console.log(`\nScan ID: ${result.scanRunId}`);
  }

  if (failOnViolation && hasViolations) {
    process.exit(1);
  }
}

// ---------------------------------------------------------------------------
// policy subcommands
// ---------------------------------------------------------------------------

function cmdPolicyList(flags: { db?: string; json: boolean }): void {
  const db = initDb(flags.db);
  const policies = listPolicies(db);
  const templates = Object.values(BUILT_IN_TEMPLATES);

  if (flags.json) {
    console.log(JSON.stringify({ policies, templates }, null, 2));
    return;
  }

  console.log('\nBuilt-in templates:');
  for (const t of templates) {
    console.log(`  ${t.name}  — ${t.description}`);
  }

  if (policies.length === 0) {
    console.log('\nNo user-defined policies.');
  } else {
    console.log(`\nUser-defined policies (${policies.length}):`);
    for (const p of policies) {
      console.log(`  ${p.name}  [${p.id}]  ${p.description ?? ''}`);
    }
  }
}

function cmdPolicyShow(nameOrId: string, flags: { db?: string; json: boolean }): void {
  const template = BUILT_IN_TEMPLATES[nameOrId];
  if (template) {
    if (flags.json) {
      console.log(JSON.stringify(template, null, 2));
    } else {
      console.log(`\nTemplate: ${template.name}`);
      console.log(`Description: ${template.description}`);
      console.log(`Rules:\n${JSON.stringify(template.rules, null, 2)}`);
    }
    return;
  }

  const db = initDb(flags.db);
  const policy = getPolicy(db, nameOrId);
  if (!policy) {
    console.error(`Policy not found: ${nameOrId}`);
    process.exit(1);
  }

  if (flags.json) {
    console.log(JSON.stringify(policy, null, 2));
    return;
  }
  console.log(`\nPolicy: ${policy.name}  [${policy.id}]`);
  if (policy.description) console.log(`Description: ${policy.description}`);
  console.log(`Rules:\n${JSON.stringify(policy.rules, null, 2)}`);
}

// ---------------------------------------------------------------------------
// Entry
// ---------------------------------------------------------------------------

const { positional, json, db, policy, failOnViolation } = parseFlags(args);

switch (command) {
  case 'scan': {
    if (!positional[0]) {
      console.error('Error: path is required');
      help();
      process.exit(2);
    }
    cmdScan(positional[0], { json, db, policy, failOnViolation }).catch(err => {
      console.error('Fatal:', err instanceof Error ? err.message : err);
      process.exit(2);
    });
    break;
  }
  case 'policy': {
    const sub = positional[0];
    if (sub === 'list') {
      cmdPolicyList({ db, json });
    } else if (sub === 'show') {
      if (!positional[1]) {
        console.error('Error: policy name or id is required');
        process.exit(1);
      }
      cmdPolicyShow(positional[1], { db, json });
    } else {
      console.error(`Unknown policy subcommand: ${sub ?? '(none)'}`);
      help();
      process.exit(2);
    }
    break;
  }
  case 'help':
  case '--help':
  case '-h':
  case undefined:
    help();
    break;
  default:
    console.error(`Unknown command: ${command}`);
    help();
    process.exit(2);
}
