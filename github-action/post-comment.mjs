#!/usr/bin/env node
/**
 * post-comment.mjs
 *
 * Reads a PHALUS JSON scan result, writes verdict outputs to $GITHUB_OUTPUT,
 * and (when running on a PR) posts a formatted markdown comment via the GitHub API.
 *
 * Called by action.yml after the scan step.
 */

import { readFileSync, appendFileSync, existsSync } from 'node:fs';
import { createRequire } from 'node:module';

// ─── Argument parsing ─────────────────────────────────────────────────────────

const args = process.argv.slice(2);

function getArg(flag) {
  const idx = args.indexOf(flag);
  return idx !== -1 && args[idx + 1] ? args[idx + 1] : '';
}

const scanFile     = getArg('--scan-file');
const stderrFile   = getArg('--stderr-file');
const policy       = getArg('--policy');
const githubOutput = getArg('--github-output');
const prNumber     = getArg('--pr-number');
const repo         = getArg('--repo');
const githubToken  = getArg('--github-token');

// ─── Read scan result ─────────────────────────────────────────────────────────

let scan = null;
let parseError = null;

try {
  if (existsSync(scanFile)) {
    const raw = readFileSync(scanFile, 'utf-8').trim();
    if (raw) scan = JSON.parse(raw);
  }
} catch (err) {
  parseError = err.message;
}

const stderrText = existsSync(stderrFile)
  ? readFileSync(stderrFile, 'utf-8').trim()
  : '';

// ─── Determine verdict ────────────────────────────────────────────────────────

let verdict = 'pass';
let violationCount = 0;
let scanId = scan?.scanRunId ?? '';

if (!scan || scan.error || parseError) {
  verdict = 'error';
} else {
  const violations = (scan.alerts ?? []).filter(a => a.kind === 'policy-violation');
  violationCount = violations.length;
  // Also check for policyVerdict field (added by policy engine when --policy is used)
  if (scan.policyVerdict === 'fail' || violationCount > 0) {
    verdict = 'fail';
  }
}

// ─── Write GITHUB_OUTPUT ──────────────────────────────────────────────────────

if (githubOutput) {
  appendFileSync(githubOutput, `verdict=${verdict}\n`);
  appendFileSync(githubOutput, `violation-count=${violationCount}\n`);
  appendFileSync(githubOutput, `scan-id=${scanId}\n`);
}

// ─── Post PR comment ──────────────────────────────────────────────────────────

if (!prNumber || !repo || !githubToken) {
  // Not running on a PR or missing token — skip comment
  process.exit(0);
}

const [owner, repoName] = repo.split('/');

function buildComment() {
  if (verdict === 'error') {
    const errMsg = scan?.error ?? parseError ?? 'Unknown scan error';
    return [
      '## PHALUS License Scan',
      '',
      '**Status**: ❌ Error',
      '',
      `> ${errMsg}`,
      stderrText ? `\n\`\`\`\n${stderrText.slice(0, 1000)}\n\`\`\`` : '',
    ].join('\n');
  }

  const packages = scan?.packages ?? [];
  const alerts   = scan?.alerts ?? [];
  const policyLine = policy ? ` (policy: \`${policy}\`)` : '';
  const statusIcon = verdict === 'fail' ? '❌' : '✅';
  const statusText = verdict === 'fail'
    ? `Fail — ${violationCount} violation(s)`
    : 'Pass';

  // Build violation rows (policy-violation alerts)
  const violations = alerts.filter(a => a.kind === 'policy-violation');
  const violationRows = violations
    .slice(0, 20)
    .map(a => `| — | — | — | ${a.message} |`)
    .join('\n');

  // Build package table (top 20, violations first)
  const pkgRows = packages
    .slice(0, 20)
    .map(p => {
      const hasViolation = violations.some(v => v.message.includes(p.name));
      return `| ${p.name} | ${p.version} | ${p.licenseExpression ?? 'unknown'} | ${hasViolation ? '⚠ violation' : '—'} |`;
    })
    .join('\n');

  const allPkgRows = packages
    .map(p => `| ${p.name} | ${p.version} | ${p.licenseExpression ?? 'unknown'} | ${p.licenseCategory} |`)
    .join('\n');

  return [
    '## PHALUS License Scan',
    '',
    `**Status**: ${statusIcon} ${statusText}${policyLine}`,
    '',
    '| Package | Version | License | Violation |',
    '|---------|---------|---------|-----------|',
    pkgRows || '| — | — | — | — |',
    '',
    `<details><summary>All packages scanned (${packages.length})</summary>`,
    '',
    '| Package | Version | License | Category |',
    '|---------|---------|---------|----------|',
    allPkgRows,
    '',
    '</details>',
    '',
    scanId ? `*Scan ID: \`${scanId}\`*` : '',
  ].filter(l => l !== undefined).join('\n');
}

const body = buildComment();

const apiUrl = `https://api.github.com/repos/${owner}/${repoName}/issues/${prNumber}/comments`;

const response = await fetch(apiUrl, {
  method: 'POST',
  headers: {
    Authorization: `Bearer ${githubToken}`,
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({ body }),
});

if (!response.ok) {
  const text = await response.text();
  console.error(`Failed to post PR comment: ${response.status} ${text}`);
  // Don't fail the action just because the comment failed
}

process.exit(0);
