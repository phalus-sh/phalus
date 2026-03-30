# PHALUS — Private Headless Automated License Uncoupling System

PHALUS scans your project dependencies for license compliance issues and can block CI pipelines on violations.

## Quick Start

```bash
# Install
npm install -g @phalus/cli

# Scan the current directory
phalus scan .

# Scan with a policy and fail on violations
phalus scan . --policy permissive-only

# Output JSON (useful for CI scripting)
phalus scan . --json
```

## Exit Codes

| Code | Meaning |
|------|---------|
| `0`  | Clean scan — no violations, or policy check passed |
| `1`  | Policy violations found |
| `2`  | Scan error — invalid path, parse failure, or fatal error |

## `.phalus.yml` Config File

Place a `.phalus.yml` file in your repository root to configure PHALUS defaults for CI. CLI flags override file values.

```yaml
policy: permissive-only  # policy name or ID
failOnViolation: true    # exit 1 on violations (default: true when policy set)
paths:
  - .                    # directories to scan
ecosystem: auto          # or explicit: npm, pip, cargo, go
```

### Built-in Policies

| Policy | Description |
|--------|-------------|
| `permissive-only` | Allow only permissive licenses (MIT, Apache-2.0, BSD, ISC, …) |
| `no-copyleft-strong` | Deny strong copyleft licenses (GPL, AGPL, …) |
| `no-proprietary` | Deny proprietary / commercial licenses |

## GitHub Action

Use the PHALUS GitHub Action to scan your dependencies on every pull request and block merges on violations.

### Basic usage

```yaml
# .github/workflows/license-scan.yml
name: License Scan

on:
  pull_request:
    branches: [main]

jobs:
  phalus:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: phalus-sh/phalus/github-action@v1
        with:
          policy: permissive-only
```

### Inputs

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `policy` | No | — | Policy name or ID to enforce |
| `path` | No | `.` | Directory to scan |
| `fail-on-violation` | No | `true` | Exit 1 when violations found (when policy set) |
| `api-url` | No | — | PHALUS API server URL (for server-backed mode) |
| `api-key` | No | — | API key for server-backed mode |
| `phalus-version` | No | `latest` | Version of `@phalus/cli` to install |

### Outputs

| Output | Description |
|--------|-------------|
| `verdict` | `pass`, `fail`, or `error` |
| `violation-count` | Number of policy violations |
| `scan-id` | PHALUS scan run ID |

### Full example with PR comment

The action automatically posts a scan summary comment on pull requests when `GITHUB_TOKEN` is available.

```yaml
- uses: phalus-sh/phalus/github-action@v1
  with:
    policy: no-copyleft-strong
    path: .
    fail-on-violation: true
```

The PR comment format:

```
## PHALUS License Scan

**Status**: ❌ Fail — 2 violation(s) (policy: `no-copyleft-strong`)

| Package | Version | License | Violation |
|---------|---------|---------|-----------|
| some-pkg | 1.0.0 | GPL-3.0 | ⚠ violation |

<details><summary>All packages scanned (42)</summary>
...
</details>
```

## CLI Reference

```
phalus <command> [options]

Commands:
  scan <path>    Scan a project directory for license data
  help           Show this help

Options:
  --db <path>              SQLite database path (default: ./phalus.db or $PHALUS_DB_PATH)
  --json                   Output results as JSON
  --policy <name>          Policy name or ID to enforce
  --fail-on-violation      Exit 1 if policy violations are found (default when --policy set)
  --no-fail-on-violation   Never exit 1 due to policy violations
```

## Supported Ecosystems

| Ecosystem | Files scanned |
|-----------|--------------|
| npm | `package-lock.json`, `package.json` |
| pip | `requirements.txt`, `pyproject.toml` |
| cargo | `Cargo.lock`, `Cargo.toml` |
| go | `go.mod` |
| SBOM | CycloneDX JSON, SPDX JSON |

## REST API

Start the API server:

```bash
PHALUS_API_KEY=your-key phalus-api
# or via Docker:
docker compose up
```

Endpoints:

- `POST /scans` — trigger a scan
- `GET /scans/:id` — get scan results
- `GET /licenses` — query packages by license
- `GET /health` — health check

See `docs/openapi.json` for the full OpenAPI 3.1 spec.
