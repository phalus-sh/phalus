#!/usr/bin/env node
import path from 'node:path';
import { initDb, createScanRun, runScan, classifyLicense } from '@phalus/core';

const [, , command, ...args] = process.argv;

function help(): void {
  console.log(`
phalus <command> [options]

Commands:
  scan <path>    Scan a project directory for license data
  help           Show this help

Options:
  --db <path>    SQLite database path (default: ./phalus.db or $PHALUS_DB_PATH)
  --json         Output results as JSON

Examples:
  phalus scan ./my-project
  phalus scan /path/to/repo --json
`);
}

async function cmdScan(targetPath: string, flags: { json: boolean; db?: string }): Promise<void> {
  const resolvedPath = path.resolve(targetPath);
  const db = initDb(flags.db);
  const scanRunId = createScanRun(db, resolvedPath);

  if (!flags.json) {
    console.log(`Scanning ${resolvedPath} ...`);
  }

  const result = await runScan(db, scanRunId, resolvedPath);

  if (result.error) {
    if (flags.json) {
      console.log(JSON.stringify({ error: result.error }, null, 2));
    } else {
      console.error(`\nError: ${result.error}`);
    }
    process.exit(1);
  }

  if (flags.json) {
    console.log(JSON.stringify(result, null, 2));
    return;
  }

  // Human-readable output
  const { packages, alerts } = result;
  console.log(`\nFound ${packages.length} package(s)\n`);

  // Group by ecosystem
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
  console.log(`\nScan ID: ${result.scanRunId}`);
}

// Parse flags
function parseFlags(rawArgs: string[]): { positional: string[]; json: boolean; db?: string } {
  const positional: string[] = [];
  let json = false;
  let db: string | undefined;
  for (let i = 0; i < rawArgs.length; i++) {
    if (rawArgs[i] === '--json') {
      json = true;
    } else if (rawArgs[i] === '--db' && rawArgs[i + 1]) {
      db = rawArgs[++i];
    } else {
      positional.push(rawArgs[i]!);
    }
  }
  return { positional, json, db };
}

const { positional, json, db } = parseFlags(args);

switch (command) {
  case 'scan': {
    if (!positional[0]) {
      console.error('Error: path is required');
      help();
      process.exit(1);
    }
    cmdScan(positional[0], { json, db }).catch(err => {
      console.error('Fatal:', err instanceof Error ? err.message : err);
      process.exit(1);
    });
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
    process.exit(1);
}
