# Getting Started

## Prerequisites

| Option | Requirement |
|--------|-------------|
| Build from source | Rust 1.78 or later (`rustup update stable`) |
| Pre-built binary | None — download and run |
| Docker | Docker Engine 24 or later |

You also need at least one LLM API key. Both agents default to `claude-sonnet-4-6` (Anthropic), but each agent can be configured independently and can use any supported provider.

---

## Install

### Option 1 — cargo install

```bash
cargo install phalus
```

Verify the installation:

```bash
phalus --version
```

### Option 2 — Pre-built binary

Download the appropriate archive from the [releases page](https://github.com/phalus-project/phalus/releases):

```bash
# Linux x86_64 example
curl -L https://github.com/phalus-project/phalus/releases/latest/download/phalus-linux-x86_64.tar.gz \
  | tar xz
sudo mv phalus /usr/local/bin/
phalus --version
```

### Option 3 — Docker

```bash
docker pull ghcr.io/phalus-project/phalus:latest
```

For convenience, add a shell alias:

```bash
alias phalus='docker run --rm \
  -e PHALUS_LLM__AGENT_A_API_KEY \
  -e PHALUS_LLM__AGENT_B_API_KEY \
  -v "$PWD":/work -w /work \
  ghcr.io/phalus-project/phalus:latest'
```

---

## Configure API Keys

PHALUS requires separate API keys for Agent A (Analyzer) and Agent B (Builder). Using separate keys provides stronger isolation evidence, though the same key works.

**Environment variables (recommended for getting started):**

```bash
export PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..."
export PHALUS_LLM__AGENT_B_API_KEY="sk-ant-..."
```

**Config file (recommended for regular use):**

```bash
mkdir -p ~/.phalus
cat > ~/.phalus/config.toml <<'EOF'
[llm]
agent_a_provider = "anthropic"
agent_a_model    = "claude-sonnet-4-6"
agent_a_api_key  = "sk-ant-..."

agent_b_provider = "anthropic"
agent_b_model    = "claude-sonnet-4-6"
agent_b_api_key  = "sk-ant-..."
EOF
```

Verify that the configuration is loaded correctly (API keys are always redacted in this output):

```bash
phalus config
```

---

## Quick Start

### Run a single package

The `run-one` command is the fastest way to try PHALUS. It does not require a manifest file.

```bash
phalus run-one npm/left-pad@1.1.3 --license mit
```

The format is `ecosystem/name@version`. Supported ecosystems: `npm`, `pypi`, `crates`, `go`.

You should see output similar to:

```
OK left-pad@1.1.3
```

### Inspect the output

```bash
phalus inspect ./phalus-output --csp --similarity --audit
```

**CSP section** lists the ten specification documents Agent A produced:

```
=== CSP Specs ===
  left-pad@1.1.3 (10 documents)
    - 01-overview.md
    - 02-api-surface.json
    - 03-behavior-spec.md
    - 04-edge-cases.md
    - 05-configuration.md
    - 06-type-definitions.d.ts
    - 07-error-catalog.md
    - 08-compatibility-notes.md
    - 09-test-scenarios.md
    - 10-metadata.json
```

**Similarity section** shows how close the generated code is to the original (lower is better for the clean room claim):

```
=== Similarity Reports ===
  left-pad@1.1.3:
    token_similarity: 0.1800
    name_overlap:     0.9000
    string_overlap:   0.1200
    overall_score:    0.2500
    verdict:          PASS
```

Note: name overlap is intentionally high — the public API names must match by design.

**Audit section** shows the event log for the run:

```
=== Audit Log ===
  [2026-03-26T10:00:00Z] seq=0 type=manifest_parsed
  [2026-03-26T10:00:01Z] seq=1 type=docs_fetched
  [2026-03-26T10:00:03Z] seq=2 type=spec_generated
  [2026-03-26T10:00:03Z] seq=3 type=firewall_crossing
  [2026-03-26T10:00:08Z] seq=4 type=implementation_generated
  [2026-03-26T10:00:09Z] seq=5 type=validation_completed
```

### Run from a manifest

```bash
# Preview what would be processed
phalus plan package.json

# Run the full pipeline
phalus run package.json --license apache-2.0 --output ./output/
```

---

## Output Structure

After a successful run, the output directory contains:

```
phalus-output/
├── left-pad/
│   ├── package.json
│   ├── LICENSE
│   ├── README.md
│   ├── src/
│   │   └── index.js
│   ├── test/
│   │   └── index.test.js
│   ├── validation.json          # similarity + verdict
│   └── .cleanroom/
│       └── csp/
│           ├── 01-overview.md
│           ├── 02-api-surface.json
│           └── ...
└── audit.jsonl                  # job-level audit trail
```

---

## Split pipeline: Agent A and Agent B separately

You can run Agent A (spec generation) and Agent B (code implementation) as separate steps. This allows you to review, edit, or programmatically modify the specification before building.

```bash
# Step 1: Generate CSP only (Agent A)
phalus run-one npm/lodash@4.17.21 --dry-run

# Step 2: Review the specification
phalus inspect ./phalus-output --csp
cat ./phalus-output/lodash/.cleanroom/csp/03-behavior-spec.md

# Step 3: Build from the CSP (Agent B)
phalus build ./phalus-output/lodash/.cleanroom/csp/
```

See the [Cookbook](cookbook.md) for advanced workflows including injecting custom security constraints and batch processing with review gates.

---

## Next Steps

- [Cookbook — split pipeline, CSP modification, and automation recipes](cookbook.md)
- [Pipeline explained in detail](pipeline.md)
- [Full CLI reference](cli-reference.md)
- [Configuration reference](configuration.md)
- [Docker guide](docker.md)
