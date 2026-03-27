# Cookbook

Recipes for common PHALUS workflows beyond the basic `run` / `run-one` commands. These examples demonstrate how to use the split pipeline, modify specifications, and automate custom workflows.

---

## Generate a CSP from a public package (Agent A only)

Use `--dry-run` to run only Agent A (the Analyzer). This fetches documentation from the package's public registry and repository, produces the 10-document Clean Room Specification Pack, and stops before Agent B generates any code.

### Single package

```bash
# Generate a CSP for a specific npm package
phalus run-one npm/lodash@4.17.21 --dry-run

# Generate a CSP for a Python package
phalus run-one pypi/requests@2.31.0 --dry-run

# Generate a CSP for a Rust crate
phalus run-one crates/serde@1.0.193 --dry-run

# Generate a CSP for a Go module
phalus run-one go/github.com/gin-gonic/gin@v1.9.1 --dry-run
```

The package is specified as `ecosystem/name@version`. PHALUS resolves the package from its public registry (npmjs.org, pypi.org, crates.io, proxy.golang.org), fetches documentation from the repository URL listed in the registry metadata, and passes that documentation to Agent A.

### From a manifest (multiple packages)

```bash
# Generate CSPs for all packages in a manifest
phalus run package.json --dry-run

# Generate CSPs for specific packages only
phalus run package.json --dry-run --only lodash,chalk
```

### Output

After `--dry-run`, the CSP is written to disk at:

```
phalus-output/
└── <package-name>/
    └── .cleanroom/
        └── csp/
            ├── manifest.json           # Machine-readable CSP manifest
            ├── 01-overview.md          # Package purpose and scope
            ├── 02-api-surface.json     # Complete public API signatures
            ├── 03-behavior-spec.md     # Behavioural specification per function
            ├── 04-edge-cases.md        # Edge cases and error conditions
            ├── 05-configuration.md     # Options and defaults
            ├── 06-type-definitions.d.ts # TypeScript-style type definitions
            ├── 07-error-catalog.md     # Error types and codes
            ├── 08-compatibility-notes.md # Platform compatibility
            ├── 09-test-scenarios.md    # Black-box test cases
            └── 10-metadata.json        # Package metadata and timestamp
```

The `manifest.json` file is the machine-readable index used by `phalus build`. The individual `.md` and `.json` files are the human-readable specification documents.

### Inspect the generated CSP

```bash
phalus inspect ./phalus-output --csp
```

---

## Build from an existing CSP (Agent B only)

Use `phalus build` to run only Agent B (the Builder) from a pre-existing CSP. Agent B reads only the CSP documents and implements the package from scratch. It never sees the original source code or documentation.

### Basic usage

```bash
# Point to the CSP directory
phalus build ./phalus-output/lodash/.cleanroom/csp/

# Or point directly to the manifest.json
phalus build ./phalus-output/lodash/.cleanroom/csp/manifest.json
```

### With options

```bash
# Specify a different license
phalus build ./phalus-output/lodash/.cleanroom/csp/ --license apache-2.0

# Build in a different language
phalus build ./phalus-output/lodash/.cleanroom/csp/ --target-lang rust

# Use process-level isolation and write to a custom directory
phalus build ./phalus-output/lodash/.cleanroom/csp/ \
  --isolation process \
  --output ./rust-output/
```

### Reuse a CSP for multiple languages

Generate one CSP, then build implementations in several languages:

```bash
# Step 1: Generate the CSP once
phalus run-one npm/chalk@5.3.0 --dry-run

# Step 2: Build in multiple languages
phalus build ./phalus-output/chalk/.cleanroom/csp/ --target-lang typescript --output ./ts-output/
phalus build ./phalus-output/chalk/.cleanroom/csp/ --target-lang rust --output ./rust-output/
phalus build ./phalus-output/chalk/.cleanroom/csp/ --target-lang python --output ./py-output/
phalus build ./phalus-output/chalk/.cleanroom/csp/ --target-lang go --output ./go-output/
```

---

## Modify the CSP before building

The split pipeline (`--dry-run` then `build`) allows you to inspect, edit, or programmatically modify the CSP between Agent A and Agent B. Since the CSP is a set of plain-text files on disk, you can use any tool to modify them.

### Manual review and editing

```bash
# Step 1: Generate the CSP
phalus run-one npm/express@4.18.2 --dry-run

# Step 2: Review the specification
cat ./phalus-output/express/.cleanroom/csp/03-behavior-spec.md
cat ./phalus-output/express/.cleanroom/csp/04-edge-cases.md

# Step 3: Edit with any text editor
$EDITOR ./phalus-output/express/.cleanroom/csp/03-behavior-spec.md

# Step 4: Build from the modified CSP
phalus build ./phalus-output/express/.cleanroom/csp/
```

### Inject custom security constraints

To add security requirements before Agent B begins implementation, edit the relevant CSP documents. Agent B treats the CSP as its sole source of truth, so any constraints you add will be reflected in the generated code.

**Which CSP documents to edit:**

| Document | What to add |
|----------|-------------|
| `03-behavior-spec.md` | Input validation rules, sanitization requirements, authentication/authorization behaviour |
| `04-edge-cases.md` | Security-relevant edge cases (e.g. path traversal, injection attacks, overflow handling) |
| `07-error-catalog.md` | Security error types (e.g. `AuthenticationError`, `PermissionDenied`) |
| `09-test-scenarios.md` | Security-focused test cases (e.g. XSS payloads, SQL injection strings) |

**Example: Adding input validation to the behaviour spec**

```bash
# Generate the CSP
phalus run-one npm/my-parser@2.0.0 --dry-run

# Append security constraints to the behaviour spec
cat >> ./phalus-output/my-parser/.cleanroom/csp/03-behavior-spec.md << 'EOF'

## Security Constraints

All public functions that accept string input MUST:
- Reject input exceeding 1 MB in length with a `InputTooLargeError`
- Sanitize any HTML entities in string output
- Never include raw user input in error messages

All file path parameters MUST:
- Reject paths containing `..` components
- Resolve symlinks and verify the target is within the expected base directory
EOF

# Build with the modified spec
phalus build ./phalus-output/my-parser/.cleanroom/csp/
```

### Programmatic CSP modification

The CSP `manifest.json` is a standard JSON file. You can parse and modify it with any JSON tool (`jq`, Python, Node.js, etc.) for automated pipelines.

**manifest.json structure:**

```json
{
  "package_name": "lodash",
  "package_version": "4.17.21",
  "generated_at": "2026-03-27T10:00:03Z",
  "documents": [
    {
      "filename": "01-overview.md",
      "content": "# lodash\n\nA modern JavaScript utility library...",
      "content_hash": "a3f2c1d4..."
    },
    {
      "filename": "03-behavior-spec.md",
      "content": "...",
      "content_hash": "b5c6d7e8..."
    }
  ]
}
```

**Example: Inject constraints with jq**

```bash
# Generate CSP
phalus run-one npm/lodash@4.17.21 --dry-run

CSP_DIR="./phalus-output/lodash/.cleanroom/csp"

# Append security constraints to 03-behavior-spec.md via jq
jq '(.documents[] | select(.filename == "03-behavior-spec.md") | .content) += "\n\n## Security\nAll functions must validate input types at runtime.\n"' \
  "$CSP_DIR/manifest.json" > "$CSP_DIR/manifest.json.tmp" \
  && mv "$CSP_DIR/manifest.json.tmp" "$CSP_DIR/manifest.json"

# Build from the modified CSP
phalus build "$CSP_DIR"
```

**Example: Python script to inject constraints**

```python
#!/usr/bin/env python3
"""Inject custom constraints into a PHALUS CSP manifest."""

import json
import sys

csp_path = sys.argv[1]  # e.g. ./phalus-output/lodash/.cleanroom/csp/manifest.json

with open(csp_path) as f:
    csp = json.load(f)

# Define custom security constraints
security_constraints = """

## Security Constraints (Injected)

- All functions accepting user input MUST validate argument types at runtime.
- String inputs longer than 10 MB MUST be rejected with an appropriate error.
- Functions MUST NOT throw exceptions that leak internal implementation details.
- All numeric inputs MUST be checked for NaN and Infinity.
"""

# Inject into the behaviour spec
for doc in csp["documents"]:
    if doc["filename"] == "03-behavior-spec.md":
        doc["content"] += security_constraints
        break

# Add security-focused test scenarios
for doc in csp["documents"]:
    if doc["filename"] == "09-test-scenarios.md":
        doc["content"] += """

## Security Tests (Injected)

- Passing `null` or `undefined` to any public function should not throw an unhandled exception.
- Passing a string of 20 MB to string-accepting functions should return an error, not hang.
- Passing `NaN` or `Infinity` to numeric parameters should be handled gracefully.
"""
        break

with open(csp_path, "w") as f:
    json.dump(csp, f, indent=2)

print(f"Injected security constraints into {csp_path}")
```

Usage:

```bash
phalus run-one npm/lodash@4.17.21 --dry-run
python3 inject-constraints.py ./phalus-output/lodash/.cleanroom/csp/manifest.json
phalus build ./phalus-output/lodash/.cleanroom/csp/
```

---

## Full split pipeline: end-to-end example

This example demonstrates the complete split workflow — generating a CSP, reviewing it, injecting custom constraints, building the implementation, and validating the output.

```bash
# 1. Generate the CSP (Agent A only)
phalus run-one npm/express@4.18.2 --dry-run --output ./cleanroom/

# 2. Inspect what Agent A produced
phalus inspect ./cleanroom/ --csp

# 3. Review the specification
cat ./cleanroom/express/.cleanroom/csp/03-behavior-spec.md

# 4. Inject custom security constraints
cat >> ./cleanroom/express/.cleanroom/csp/04-edge-cases.md << 'EOF'

## Custom: Request Size Limits
- Requests with bodies exceeding `max_body_size` (default 1 MB) MUST be rejected with HTTP 413.
- Header values exceeding 8 KB MUST be rejected.
- URL paths exceeding 2048 characters MUST be rejected.
EOF

# 5. Build the implementation (Agent B only)
phalus build ./cleanroom/express/.cleanroom/csp/ \
  --license apache-2.0 \
  --output ./cleanroom/

# 6. Validate the output
phalus validate ./cleanroom/

# 7. Inspect similarity scores and audit trail
phalus inspect ./cleanroom/ --similarity --audit
```

---

## Automate with shell scripts

### Batch process with CSP review gate

```bash
#!/usr/bin/env bash
set -euo pipefail

MANIFEST="$1"
OUTPUT="./phalus-output"

# Phase 1: Generate all CSPs
echo "=== Phase 1: Generating specifications ==="
phalus run "$MANIFEST" --dry-run --output "$OUTPUT"

# Phase 2: Review gate
echo "=== Phase 2: CSP Review ==="
echo "CSPs written to $OUTPUT/*/. cleanroom/csp/"
echo "Review and modify as needed, then press Enter to continue."
read -r

# Phase 3: Build each package from its CSP
echo "=== Phase 3: Building implementations ==="
for csp_dir in "$OUTPUT"/*/.cleanroom/csp/; do
    if [ -f "$csp_dir/manifest.json" ]; then
        echo "Building from $csp_dir"
        phalus build "$csp_dir" --output "$OUTPUT"
    fi
done

# Phase 4: Validate
echo "=== Phase 4: Validation ==="
phalus validate "$OUTPUT"
```

Usage:

```bash
chmod +x batch-build.sh
./batch-build.sh package.json
```

---

## Use the REST API for the same workflows

The web server (`phalus serve`) exposes the same pipeline. For programmatic access over HTTP:

### Start a job (equivalent to `run`)

```bash
# Start the server
phalus serve &

# Submit a job
JOB_ID=$(curl -s -X POST http://127.0.0.1:3000/api/jobs \
  -H 'Content-Type: application/json' \
  -d '{"manifest_content": "{\"dependencies\":{\"lodash\":\"^4.17.21\"}}", "license": "mit"}' \
  | jq -r '.job_id')

# Stream progress
curl -N "http://127.0.0.1:3000/api/jobs/$JOB_ID/stream"
```

### Retrieve a CSP via the API

```bash
# After the job completes, fetch the CSP
curl -s "http://127.0.0.1:3000/api/packages/lodash/csp" | jq .
```

### Retrieve generated code

```bash
curl -s "http://127.0.0.1:3000/api/packages/lodash/code" | jq .
```

See the [API Reference](api-reference.md) for the full endpoint list.
