# CLI Reference

PHALUS is CLI-first. Every capability is accessible from the command line; the web UI is a convenience wrapper over the same pipeline.

```
phalus [COMMAND] [OPTIONS]
```

Run `phalus --help` for a summary or `phalus [COMMAND] --help` for per-command help.

---

## Commands

| Command | Description |
|---------|-------------|
| [`plan`](#plan) | Parse a manifest and show what would be processed |
| [`run`](#run) | Run the full clean room pipeline from a manifest |
| [`run-one`](#run-one) | Run the pipeline on a single package without a manifest |
| [`build`](#build) | Run Agent B only from an existing CSP |
| [`scan`](#scan) | Scan a project for dependency licenses and SBOM data |
| [`inspect`](#inspect) | Inspect a completed job's output directory |
| [`validate`](#validate) | Re-run validation on an existing output directory |
| [`config`](#config) | Print the active configuration (API keys redacted) |
| [`serve`](#serve) | Start the local web UI |

---

## plan

Parse a manifest file and display the packages that would be processed. No network calls to registries or LLMs are made. Use this to preview filtering before a long run.

```
phalus plan <MANIFEST> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `MANIFEST` | Path to the manifest file (e.g. `package.json`, `requirements.txt`, `Cargo.toml`, `go.mod`) |

### Options

| Option | Description |
|--------|-------------|
| `--only <pkg1,pkg2,...>` | Process only these packages (comma-separated names) |
| `--exclude <pkg1,pkg2,...>` | Skip these packages (comma-separated names) |

### Example

```bash
phalus plan package.json

phalus plan package.json --only lodash,chalk

phalus plan requirements.txt --exclude boto3,botocore
```

**Output:**

```
Manifest: package.json (5 packages, 3 after filtering)

PACKAGE                        VERSION          ECOSYSTEM
-------------------------------------------------------
lodash                         ^4.17.21         npm
express                        ^4.18.2          npm
chalk                          ^5.3.0           npm
```

---

## run

Run the full clean room pipeline for all packages in a manifest. Packages are processed concurrently up to the configured limit.

```
phalus run <MANIFEST> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `MANIFEST` | Path to the manifest file |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--license <id>` | `mit` | SPDX license identifier for the generated code. Options: `mit`, `apache-2.0`, `bsd-2`, `bsd-3`, `isc`, `unlicense`, `cc0` |
| `--license-file <path>` | — | Path to a file containing full license text (overrides `--license`) |
| `--output <dir>` | `./phalus-output` | Directory to write generated packages |
| `--only <pkg1,pkg2,...>` | — | Process only these packages |
| `--exclude <pkg1,pkg2,...>` | — | Skip these packages |
| `--target-lang <lang>` | Same as source | Reimplement in a different language: `rust`, `go`, `python`, `typescript` |
| `--isolation <mode>` | `context` | Isolation strategy: `context`, `process`, `container` |
| `--similarity-threshold <f>` | `0.70` | Similarity score above which a package is flagged as FAIL |
| `--concurrency <n>` | `3` | Number of packages to process in parallel |
| `--dry-run` | false | Run Agent A only — produce CSP specs, skip Agent B and validation. Use with [`build`](#build) for a split workflow. |
| `--verbose` | false | Enable verbose logging |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | All packages processed successfully |
| `1` | One or more packages failed |

### Examples

```bash
# Basic run, MIT license
phalus run package.json

# Apache-2.0 output into a specific directory
phalus run package.json --license apache-2.0 --output ./liberated/

# Process only two packages
phalus run package.json --only lodash,express

# Reimplement JavaScript packages in Rust
phalus run package.json --target-lang rust --license apache-2.0

# Dry run: trigger Agent A only, produce CSP specs without code generation
phalus run package.json --dry-run

# Dry run a single package, then build from the CSP later
phalus run-one npm/lodash@4.17.21 --dry-run
phalus build ./phalus-output/lodash/.cleanroom/csp/

# Stricter similarity threshold
phalus run package.json --similarity-threshold 0.50

# Stronger isolation mode
phalus run package.json --isolation process
```

---

## run-one

Run the clean room pipeline on a single package without needing a manifest file. Useful for quick experiments and testing a specific package.

```
phalus run-one <PACKAGE> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `PACKAGE` | Package specification in the format `ecosystem/name@version`. For example: `npm/lodash@4.17.21`, `pypi/requests@2.31.0`, `crates/serde@1.0.193`, `go/github.com/gin-gonic/gin@v1.9.1` |

Supported ecosystems: `npm`, `pypi`, `crates`, `go`.

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--license <id>` | `mit` | SPDX license identifier |
| `--license-file <path>` | — | Path to a file containing full license text (overrides `--license`) |
| `--output <dir>` | `./phalus-output` | Output directory |
| `--target-lang <lang>` | Same as source | Target language: `rust`, `go`, `python`, `typescript` |
| `--isolation <mode>` | `context` | Isolation mode: `context`, `process`, `container` |
| `--similarity-threshold <f>` | `0.70` | Similarity threshold |
| `--verbose` | false | Enable verbose logging |

### Examples

```bash
phalus run-one npm/left-pad@1.1.3

phalus run-one npm/lodash@4.17.21 --license apache-2.0

phalus run-one pypi/requests@2.31.0 --output ./py-output/

phalus run-one npm/chalk@5.3.0 --target-lang rust --license mit

phalus run-one crates/serde@1.0.193 --isolation process
```

---

## build

Run Agent B (Builder) from an existing Clean Room Specification Pack without re-running Agent A. This is the second half of the split pipeline workflow:

1. Generate a CSP with `--dry-run` (Agent A only)
2. Optionally review or modify the CSP files on disk
3. Run `build` to implement from the CSP (Agent B only)

```
phalus build <CSP> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `CSP` | Path to a CSP `manifest.json` file, **or** a directory containing one (e.g. `./phalus-output/lodash/.cleanroom/csp/`) |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--license <id>` | `mit` | SPDX license identifier for the generated code |
| `--license-file <path>` | — | Path to a file containing full license text (overrides `--license`) |
| `--output <dir>` | `./phalus-output` | Output directory |
| `--target-lang <lang>` | Same as source | Target language: `rust`, `go`, `python`, `typescript` |
| `--isolation <mode>` | `context` | Isolation mode: `context`, `process`, `container` |
| `--similarity-threshold <f>` | `0.70` | Similarity threshold |
| `--verbose` | false | Enable verbose logging |

### Examples

```bash
# Build from a CSP directory (looks for manifest.json inside)
phalus build ./phalus-output/lodash/.cleanroom/csp/

# Build from a specific manifest.json
phalus build ./my-specs/lodash-csp/manifest.json --license apache-2.0

# Build in a different language
phalus build ./phalus-output/chalk/.cleanroom/csp/ --target-lang rust

# Full split workflow: generate CSP, review it, then build
phalus run-one npm/express@4.18.2 --dry-run
# ... inspect and optionally edit the CSP files ...
phalus build ./phalus-output/express/.cleanroom/csp/ --license mit
```

### Split pipeline workflow

The `build` command enables a split pipeline where Agent A and Agent B run as separate steps. This is useful for:

- **Reviewing the CSP** before committing to implementation
- **Injecting custom constraints** into the specification (e.g. security requirements in `03-behavior-spec.json` or `04-edge-cases.json`)
- **Reusing a CSP** to generate implementations in multiple languages
- **Iterating on the implementation** without re-running analysis

See the [Cookbook](cookbook.md) for detailed examples of these workflows.

---

## scan

Scan a project directory, manifest file, or SBOM file for dependency licenses. Identifies all dependencies, resolves their license metadata from package registries, normalizes license strings to SPDX identifiers, and classifies them into risk buckets.

Supports reading manifests (`package.json`, `requirements.txt`, `Cargo.toml`, `go.mod`) and SBOM files (CycloneDX JSON 1.4+, SPDX JSON 2.3+).

```
phalus scan <PATH> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `PATH` | Directory to walk, manifest file, or SBOM file (e.g. `bom.json`, `sbom.json`) |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--offline` | false | Skip registry lookups; report only what manifests and SBOMs declare locally |
| `--concurrency <n>` | `8` | Maximum concurrent registry lookups |
| `--output <format>` | `text` | Output format: `text` (human-readable table) or `json` |
| `--save` | false | Persist the scan result to `~/.phalus/scans/{id}.json` for later retrieval |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Scan completed successfully |
| `1` | Scan failed (invalid path, parse error, etc.) |

### License classification

Each dependency license is normalized to an SPDX identifier and classified:

| Class | Examples | Risk |
|-------|----------|------|
| Permissive | MIT, Apache-2.0, BSD-2-Clause, ISC | Low |
| Copyleft (weak) | LGPL-2.1, MPL-2.0, EPL-2.0 | Medium |
| Copyleft (strong) | GPL-2.0, GPL-3.0, AGPL-3.0 | High |
| Proprietary | Commercial, All Rights Reserved | Review required |
| Unknown | Unrecognized license strings | Review required |

### Supported SBOM formats

| Format | Detection |
|--------|-----------|
| CycloneDX JSON (1.4+) | Files named `bom.json`, `cyclonedx.json`, or containing `bomFormat` key |
| SPDX JSON (2.3+) | Files named `sbom.json`, `spdx.json`, or containing `spdxVersion` key |

### Examples

```bash
# Scan a project directory (finds all manifests and SBOMs recursively)
phalus scan ./my-project

# Scan a single manifest
phalus scan package.json

# Scan a CycloneDX SBOM
phalus scan bom.json

# Offline scan (no registry calls)
phalus scan ./my-project --offline

# JSON output, saved for later retrieval via the API
phalus scan ./my-project --output json --save

# High concurrency for large projects
phalus scan ./monorepo --concurrency 16
```

**Sample text output:**

```
Scan: ./my-project (42 packages from 1 manifest, 0 SBOMs)

PACKAGE                VERSION     ECOSYSTEM  LICENSE         CLASS
---------------------------------------------------------------------------
lodash                 4.17.21     npm        MIT             permissive
express                4.18.2      npm        MIT             permissive
pg                     8.11.3      npm        MIT             permissive
react                  18.2.0      npm        MIT             permissive
node-forge             1.3.1       npm        BSD-3-Clause    permissive
sharp                  0.33.2      npm        Apache-2.0      permissive
readline-sync          1.4.10      npm        MIT             permissive

Summary: 40 permissive, 1 copyleft-weak, 0 copyleft-strong, 1 unknown
```

### Stored scans

When `--save` is used, the scan result is persisted to `~/.phalus/scans/{uuid}.json`. Stored scans can be listed and retrieved through the [REST API](api-reference.md#scan-endpoints).

---

## inspect

Display the contents of a completed job's output directory. With no flags, all sections are shown. Use individual flags to show only what you need.

```
phalus inspect <OUTPUT_DIR> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `OUTPUT_DIR` | Path to the output directory written by `run` or `run-one` |

### Options

| Option | Description |
|--------|-------------|
| `--audit` | Show the audit log (timestamp, sequence number, event type per entry) |
| `--similarity` | Show similarity scores and verdict for each package |
| `--csp` | Show the CSP document list for each package |

If none of `--audit`, `--similarity`, or `--csp` are given, all three sections are shown.

### Example

```bash
# Show everything
phalus inspect ./phalus-output

# Show only similarity scores
phalus inspect ./phalus-output --similarity

# Show only the CSP inventory
phalus inspect ./phalus-output --csp

# Show only the audit log
phalus inspect ./phalus-output --audit
```

**Sample similarity output:**

```
=== Similarity Reports ===
  lodash@4.17.21:
    token_similarity: 0.1200
    name_overlap:     0.8900
    string_overlap:   0.1500
    overall_score:    0.2800
    verdict:          PASS
```

---

## validate

Re-run the validation stage on an existing output directory without regenerating any code. Useful for checking outputs against a different similarity threshold, or after manually reviewing generated files.

```
phalus validate <OUTPUT_DIR> [OPTIONS]
```

### Arguments

| Argument | Description |
|----------|-------------|
| `OUTPUT_DIR` | Path to the output directory to validate |

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--similarity-threshold <f>` | `0.70` | Similarity threshold for pass/fail |

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | All packages passed validation |
| `1` | One or more packages failed |

### Example

```bash
phalus validate ./phalus-output

phalus validate ./phalus-output --similarity-threshold 0.50
```

**Sample output:**

```
PASS lodash (similarity: 0.2800, license: ok)
PASS chalk (similarity: 0.1900, license: ok)
FAIL express (similarity: 0.7200, license: ok)
```

---

## config

Print the active configuration, merging `~/.phalus/config.toml` with any `PHALUS_*` environment variable overrides. API keys are always redacted (shown as `***`).

```
phalus config
```

No options or arguments.

### Example

```bash
phalus config
```

**Sample output:**

```toml
[llm]
agent_a_provider = "anthropic"
agent_a_model = "claude-sonnet-4-6"
agent_a_api_key = "***"
agent_a_base_url = ""
agent_b_provider = "anthropic"
agent_b_model = "claude-sonnet-4-6"
agent_b_api_key = "***"
agent_b_base_url = ""

[isolation]
mode = "context"

[limits]
max_packages_per_job = 50
max_package_size_mb = 10
concurrency = 3
...
```

---

## serve

Start the local web UI. The server binds to `127.0.0.1:3000` by default and is accessible only from the local machine.

```
phalus serve [OPTIONS]
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `--host <host>` | `127.0.0.1` | Address to bind to |
| `--port <port>` | `3000` | Port to listen on |

### Example

```bash
# Default: localhost only
phalus serve

# Custom port
phalus serve --port 8080

# Bind to all interfaces (use with caution — no authentication)
phalus serve --host 0.0.0.0 --port 3000
```

After starting, open `http://127.0.0.1:3000` in a browser.

See [Web UI](web-ui.md) for a description of the interface and [API Reference](api-reference.md) for the backing REST endpoints.
