# PHALUS — Private Headless Automated License Uncoupling System

## Project Specification v1.0

**Status:** Draft
**Date:** 2026-03-24
**License:** 0BSD (project code) — output code license is user-configurable

---

## 1. Executive Summary

### 1.1 What This Is

PHALUS is a self-hosted, single-operator tool for AI-powered clean room software reimplementation. You feed it a dependency manifest (package.json, requirements.txt, Cargo.toml, etc.) and it runs a two-phase, isolation-enforced AI pipeline:

1. **Agent A (Analyzer)** reads only public documentation (README, API docs, type definitions) for each dependency and produces a formal specification — never touching source code.
2. **Agent B (Builder)**, in a completely isolated context, reads only that specification and implements the package from scratch.

The output is functionally equivalent code under whatever license you choose, with a full audit trail proving the clean room process.

No user accounts. No payments. No SaaS. You run it on your own machine with your own API keys.

### 1.2 Background & Motivation

This project replicates the core pipeline demonstrated by [Malus](https://malus.sh/), a satirical-but-functional service created by **Dylan Ayrey** and **Mike Nolan** and presented at FOSDEM 2026 in the Legal & Policy track ("Let's end open source together with this one simple trick"). Malus highlights a real and growing concern: as LLM coding capabilities improve, the cost and difficulty of clean room reimplementation has collapsed from months of engineering effort to seconds of compute. This fundamentally challenges the enforcement mechanisms that make open source licenses meaningful.

The legal foundation is established precedent — Phoenix Technologies' 1984 clean room clone of the IBM BIOS, and *Baker v. Selden* (1879): copyright protects expression, not ideas.

PHALUS strips the concept down to the essential machinery: the pipeline, the isolation, and the audit trail. No marketing site, no cute testimonials from "Dr. Heinrich Offshore." Just the tool.

### 1.3 Ethical Notice

This tool raises serious ethical and legal questions about open source sustainability. It exists for research, education, and transparent discourse — not to encourage license evasion. You are responsible for understanding the legal implications in your jurisdiction. The legality of AI-assisted clean room reimplementation is unsettled law.

---

## 2. System Architecture

### 2.1 High-Level Pipeline

```
              ┌─────────────────────────────┐
              │     CLI / Local Web UI       │
              │     phalus run manifest.json │
              └──────────────┬──────────────┘
                             │
              ┌──────────────▼──────────────┐
              │       Manifest Parser       │
              │  (npm, PyPI, Cargo, etc.)   │
              └──────────────┬──────────────┘
                             │
              ┌──────────────▼──────────────┐
              │      Registry Resolver      │
              │  Fetch metadata + size      │
              └──────────────┬──────────────┘
                             │
                    For each package:
                             │
              ┌──────────────▼──────────────┐
              │    PHASE 1: ANALYSIS        │
              │    Agent A (Reader)         │
              │                             │
              │  Reads:                     │
              │  · README.md                │
              │  · API docs / type defs     │
              │  · Package metadata         │
              │                             │
              │  Produces:                  │
              │  · CSP Specification        │
              └──────────────┬──────────────┘
                             │
              ┌──────────────▼──────────────┐
              │     ISOLATION FIREWALL      │
              │                             │
              │  · Separate LLM context     │
              │  · No shared state          │
              │  · Audit log boundary       │
              └──────────────┬──────────────┘
                             │
              ┌──────────────▼──────────────┐
              │    PHASE 2: CONSTRUCTION    │
              │    Agent B (Builder)        │
              │                             │
              │  Reads:                     │
              │  · CSP Specification ONLY   │
              │                             │
              │  Produces:                  │
              │  · Source code + tests      │
              │  · Package metadata         │
              └──────────────┬──────────────┘
                             │
              ┌──────────────▼──────────────┐
              │    PHASE 3: VALIDATION      │
              │                             │
              │  · Syntax check             │
              │  · Test execution           │
              │  · API surface check        │
              │  · Similarity scoring       │
              └──────────────┬──────────────┘
                             │
              ┌──────────────▼──────────────┐
              │     OUTPUT DIRECTORY        │
              │                             │
              │  · Packaged source          │
              │  · CSP spec pack            │
              │  · Audit trail              │
              │  · Similarity report        │
              └────────────────────────────┘
```

### 2.2 Component Inventory

| Component | Responsibility | Notes |
|-----------|---------------|-------|
| **CLI** | Primary interface — parse, run, inspect | Main entry point |
| **Local Web UI** | Optional browser-based interface | Single-user, no auth |
| **Manifest Parser** | Parse package.json, requirements.txt, Cargo.toml, etc. | Per-ecosystem parsers |
| **Registry Resolver** | Fetch metadata + size from npm, PyPI, crates.io, etc. | HTTP clients per registry |
| **Doc Fetcher** | Retrieve README, API docs, type defs (NEVER source code) | GitHub API + registry APIs |
| **Agent A (Analyzer)** | Read docs → produce CSP specification | LLM API call |
| **Isolation Firewall** | Enforce separation between Agent A and Agent B | Process/context boundary |
| **Agent B (Builder)** | Read spec → produce implementation | Separate LLM API call |
| **Validator** | Syntax check, run tests, score similarity | Language runtimes + diff tools |
| **Audit Logger** | Record every input/output at each stage | Append-only JSON log |
| **Storage** | Local filesystem | `~/.phalus/` or configurable |

No database. No queue. No Redis. Jobs run sequentially or with local concurrency. State lives on the filesystem.

---

## 3. Detailed Component Specifications

### 3.1 Manifest Parser

**Supported formats:**

| Ecosystem | Manifest File | Registry |
|-----------|--------------|----------|
| npm/Node.js | `package.json` | registry.npmjs.org |
| Python | `requirements.txt`, `pyproject.toml` | pypi.org |
| Rust | `Cargo.toml` | crates.io |
| Java/Kotlin | `pom.xml`, `build.gradle` | Maven Central |
| Go | `go.mod` | proxy.golang.org |
| Ruby | `Gemfile` | rubygems.org |
| PHP | `composer.json` | packagist.org |
| .NET | `*.csproj` / `packages.config` | nuget.org |

**Parser output schema:**

```json
{
  "manifest_type": "package.json",
  "packages": [
    {
      "name": "lodash",
      "version_constraint": "^4.17.21",
      "resolved_version": "4.17.21",
      "ecosystem": "npm",
      "registry_url": "https://registry.npmjs.org/lodash"
    }
  ]
}
```

**Configurable limits:**
- Maximum packages per job (default 50)
- Maximum unpacked size per package (default 10 MB)
- Transitive dependency resolution: off by default (opt-in via `--transitive`)

### 3.2 Registry Resolver

For each parsed package, fetches:

- **Package metadata**: name, version, description, keywords
- **Unpacked size**: for complexity estimation and cost tracking
- **License**: the original license being replaced
- **Repository URL**: for fetching documentation
- **Homepage / docs URL**: for additional documentation sources

**npm example:**
```
GET https://registry.npmjs.org/lodash/4.17.21
→ dist.unpackedSize, repository.url, license, description
```

**PyPI example:**
```
GET https://pypi.org/pypi/requests/2.31.0/json
→ info.description, info.license, info.project_urls, urls[].size
```

### 3.3 Documentation Fetcher

**Critical constraint: MUST NEVER fetch source code.**

Allowed inputs for Agent A:

- `README.md` / `README.rst` from the repository root
- Published API documentation (docs site, wiki)
- TypeScript type definition files (`.d.ts`) from DefinitelyTyped or the package
- `man` pages or CLI `--help` output
- Package registry description and metadata
- CHANGELOG / release notes (for behavioral expectations)

**Explicitly forbidden:**

- Any `.js`, `.py`, `.rs`, `.java`, `.go`, `.rb`, `.php`, `.c`, `.cpp` source files
- Test files (they reveal implementation details)
- Internal/private module code
- Build scripts that contain logic

**Implementation:**

1. Fetch README from GitHub API (`/repos/{owner}/{repo}/readme`)
2. Fetch type definitions from npm tarball (extract only `.d.ts`) or DefinitelyTyped
3. Fetch published docs via homepage URL if available
4. Strip inline code examples longer than N lines (configurable, default 10) — short API usage examples are acceptable as they demonstrate the public interface

The source code guard is a hard filter — any file matching a source code extension is rejected with an audit log entry recording the rejection. This is not configurable. The clean room claim depends on it.

### 3.4 Agent A — Analyzer (Specification Generator)

**Role:** Read documentation, produce a Clean Room Specification Pack (CSP).

**System prompt:**
```
You are a software specification writer performing clean room analysis.

You will receive ONLY public documentation for a software package:
README files, API documentation, type definitions, and package metadata.

You have NEVER seen the source code of this package.
You must NEVER attempt to reverse-engineer implementation details.

Your task: produce a complete, implementation-neutral specification
that another developer (who also has never seen the source code)
could use to build a functionally equivalent package from scratch.

Focus on:
- Public API surface (function signatures, classes, methods)
- Input/output behavior for each public function
- Edge cases documented in the README or API docs
- Configuration options and defaults
- Error handling behavior (documented error types/messages)
- Any documented performance characteristics

Do NOT include:
- Guesses about internal implementation
- Algorithm details not present in documentation
- Any code copied from examples (describe behavior in prose)
```

**CSP Output (10 documents):**

| # | Document | Contents |
|---|----------|----------|
| 1 | `01-overview.md` | Package purpose, scope, target use cases |
| 2 | `02-api-surface.json` | Complete public API: function signatures, types, return types |
| 3 | `03-behavior-spec.md` | Detailed behavioral specification per public function |
| 4 | `04-edge-cases.md` | Documented edge cases, error conditions, boundary behavior |
| 5 | `05-configuration.md` | Options, defaults, environment variables |
| 6 | `06-type-definitions.d.ts` | TypeScript-style type definitions for the public API |
| 7 | `07-error-catalog.md` | Error types, messages, codes |
| 8 | `08-compatibility-notes.md` | Platform requirements, browser/Node compat, version notes |
| 9 | `09-test-scenarios.md` | Black-box test cases derived from documentation |
| 10 | `10-metadata.json` | Original package name, version, license, size, analysis timestamp |

### 3.5 Isolation Firewall

The firewall is the legal and architectural core of the clean room claim. Agent B must never have access to:

- Agent A's raw inputs (the documentation)
- The original package's source code
- Any intermediate state from Agent A's processing
- The original package's repository

**Isolation strategies (configurable):**

| Strategy | Isolation Level | How It Works |
|----------|----------------|--------------|
| `context` | Separate API calls | Different conversation contexts, same API key. Simplest. Default. |
| `process` | Separate OS processes | Agent A and B run in forked processes with no shared memory. |
| `container` | Separate Docker containers | Agent A and B run in isolated containers with no network overlap. |

**Minimum viable isolation (default `context` mode):**
- Agent A and Agent B are separate LLM API calls with independent conversation contexts
- No shared system prompt content beyond the role definition
- The ONLY data crossing the firewall is the CSP specification documents
- All crossings are logged with SHA-256 checksums

**Firewall crossing audit entry:**
```json
{
  "timestamp": "2026-03-24T12:00:00Z",
  "package": "lodash@4.17.21",
  "direction": "A_to_B",
  "documents_transferred": ["01-overview.md", "02-api-surface.json", ...],
  "sha256_checksums": { "01-overview.md": "abc123...", ... },
  "agent_a_model": "claude-sonnet-4-6",
  "agent_b_model": "claude-sonnet-4-6",
  "isolation_mode": "context",
  "source_code_accessed": false
}
```

### 3.6 Agent B — Builder (Implementation Generator)

**Role:** Implement the package from scratch using ONLY the CSP specification.

**System prompt:**
```
You are a software developer performing a clean room implementation.

You will receive a specification for a software package. You have
NEVER seen the original source code. You have NEVER seen the
original documentation. You have ONLY this specification.

Your task: implement a functionally equivalent package from scratch.

Requirements:
- Implement every function/class/method in the API surface
- Match the behavioral specification exactly
- Handle all documented edge cases
- Include the test scenarios as runnable tests
- Use idiomatic code for the target language
- Add the specified license header to every file

You are free to choose any internal implementation approach.
The specification describes WHAT the code should do, not HOW.
```

**Output structure per package:**
```
<package-name>/
├── package.json          # or Cargo.toml, setup.py, etc.
├── LICENSE
├── README.md             # generated docs
├── src/
│   └── index.js          # or equivalent entry point
├── test/
│   └── index.test.js     # from CSP test scenarios
└── .cleanroom/
    ├── csp/              # the full CSP specification pack
    ├── audit.json        # complete audit trail
    └── similarity.json   # similarity analysis report
```

### 3.7 Validator

Post-generation validation pipeline:

1. **Syntax check** — Does the generated code parse without errors?
2. **Test execution** — Do the CSP-derived tests pass?
3. **API surface check** — Does the output export all functions/classes from the spec?
4. **License check** — Is the correct license header present on all files?
5. **Similarity scoring** — Compare output against original source:
   - Token-level Jaccard similarity
   - AST structural similarity (language-specific)
   - Function-name overlap ratio
   - Comment/string literal overlap
6. **Threshold check** — Flag if similarity exceeds configurable threshold (default 70%)

**Similarity report:**
```json
{
  "package": "lodash@4.17.21",
  "token_similarity": 0.12,
  "ast_similarity": 0.34,
  "name_overlap": 0.89,
  "string_overlap": 0.15,
  "overall_score": 0.28,
  "verdict": "PASS",
  "threshold": 0.70,
  "note": "High name overlap expected — public API names must match by design"
}
```

### 3.8 Audit Logger

Every pipeline step produces an immutable audit record. The complete trail is the legal evidence backing the clean room claim.

**Audit events:**

| Event | Data Captured |
|-------|--------------|
| `manifest_parsed` | Manifest hash, parsed packages, timestamp |
| `docs_fetched` | URLs accessed, content hashes, what was excluded |
| `source_code_blocked` | Any source files that were encountered and rejected |
| `spec_generated` | CSP document hashes, Agent A model/version, prompt hash |
| `firewall_crossing` | Documents transferred, checksums, isolation mode |
| `implementation_generated` | Output code hashes, Agent B model/version, prompt hash |
| `validation_completed` | Test results, similarity scores, pass/fail |
| `job_completed` | Final package hash, license applied, total elapsed time |

All audit entries include the timestamp, a monotonic sequence number, and are written to an append-only JSON-lines file. The entire audit log for a job is also hashed on completion for tamper detection.

---

## 4. CLI Interface

PHALUS is CLI-first. The local web UI is a convenience wrapper.

### 4.1 Commands

```bash
# Parse a manifest and show what would be processed
phalus plan <manifest-file>
  --only <pkg1,pkg2,...>        # process only these packages
  --exclude <pkg1,pkg2,...>     # skip these packages
  --transitive                  # include transitive deps

# Run clean room reimplementation
phalus run <manifest-file> [options]
  --license <license-id>        # mit, apache-2.0, bsd-2, bsd-3, isc, unlicense, cc0
  --license-file <path>         # custom license text
  --output <dir>                # output directory (default: ./phalus-output/)
  --only <pkg1,pkg2,...>
  --exclude <pkg1,pkg2,...>
  --target-lang <lang>          # reimplement in a different language (e.g., rust, go)
  --isolation <mode>            # context (default), process, container
  --similarity-threshold <0-1>  # flag threshold (default: 0.70)
  --concurrency <n>             # parallel packages (default: 3)
  --dry-run                     # run Agent A only, produce specs without implementation
  --verbose                     # detailed progress output

# Run on a single package directly (no manifest needed)
phalus run-one <ecosystem>/<package>@<version> [options]
  # e.g.: phalus run-one npm/lodash@4.17.21 --license mit

# Inspect a completed job
phalus inspect <output-dir>
  --audit                       # show full audit trail
  --similarity                  # show similarity scores
  --csp                         # show CSP spec summary

# Validate an existing output (re-run validation without regeneration)
phalus validate <output-dir>
  --similarity-threshold <0-1>

# Show configuration
phalus config
```

### 4.2 Example Session

```bash
$ phalus plan package.json
PHALUS — Private Headless Automated License Uncoupling System

Manifest: package.json (npm)
Packages found: 5

  lodash@4.17.21         1.4 MB   MIT
  express@4.18.2         210 KB   MIT
  axios@1.6.7            430 KB   MIT
  uuid@9.0.0             14 KB    MIT
  chalk@5.3.0            41 KB    MIT

Total unpacked size: 2.1 MB
Estimated tokens: ~180k (Agent A) + ~120k (Agent B)

$ phalus run package.json --license apache-2.0 --output ./liberated/
Processing 5 packages with isolation=context...

  [1/5] lodash@4.17.21
        ├─ Fetching docs.............. OK (README + types)
        ├─ Agent A (analyzing)........ OK (10 CSP docs, 12.4k tokens)
        ├─ Firewall crossing.......... OK (sha256 logged)
        ├─ Agent B (implementing)..... OK (847 lines, 8.2k tokens)
        ├─ Validating................. OK (similarity: 0.31 PASS)
        └─ Done ✓

  [2/5] express@4.18.2
        ├─ Fetching docs.............. OK
        ...

Complete. Output: ./liberated/
  5 packages processed, 0 failed
  Total tokens used: 298,412
  Audit trail: ./liberated/.phalus/audit.jsonl
```

### 4.3 Run-One Shortcut

For quick single-package jobs without a manifest:

```bash
$ phalus run-one npm/left-pad@1.1.3 --license unlicense
Processing left-pad@1.1.3...
  ├─ Fetching docs.............. OK
  ├─ Agent A (analyzing)........ OK (10 CSP docs)
  ├─ Firewall crossing.......... OK
  ├─ Agent B (implementing)..... OK (23 lines)
  ├─ Validating................. OK (similarity: 0.18 PASS)
  └─ Done ✓

Output: ./phalus-output/left-pad/
```

---

## 5. Local Web UI (Optional)

A lightweight browser interface for people who prefer not to use the CLI. Runs locally, no auth, no network-facing exposure by default.

### 5.1 Design

Single-page app served by the same process that runs the pipeline. Binds to `localhost:3000` by default.

### 5.2 Views

1. **Home** — File drop zone for manifest upload + "run one" package input
2. **Plan** — Parsed packages table showing name, version, size, original license. Checkboxes to include/exclude. License selector for output. Start button.
3. **Progress** — Live pipeline status per package (SSE). Phase indicators: fetching → analyzing → firewall → implementing → validating → done.
4. **Results** — Per-package cards: download code, view CSP spec, view similarity report, view audit trail. Bulk download as ZIP.

### 5.3 Implementation

- Served by the API process (Express or Fastify)
- Single HTML file with embedded JS/CSS, or a small React/Preact build
- SSE endpoint for progress updates: `GET /api/jobs/:id/stream`
- No auth, no sessions, no cookies — if you can reach `localhost:3000`, you're in

### 5.4 API Endpoints (backing the web UI and potentially useful standalone)

```
POST   /api/manifest/parse       # Upload manifest, get package list
POST   /api/jobs                  # Start a clean room job
GET    /api/jobs/:id              # Job status + results
GET    /api/jobs/:id/stream       # SSE progress stream
GET    /api/jobs/:id/download     # ZIP of all output packages
GET    /api/packages/:name/csp    # CSP spec for a completed package
GET    /api/packages/:name/audit  # Audit trail for a package
GET    /api/packages/:name/code   # Output source for a package
```

---

## 6. Configuration

### 6.1 Config File

`~/.phalus/config.toml` (also overridable via env vars and CLI flags):

```toml
[llm]
# Agent A configuration
agent_a_provider = "anthropic"        # anthropic | openai | ollama
agent_a_model = "claude-sonnet-4-6"
agent_a_api_key = "sk-ant-..."
agent_a_base_url = ""                 # optional, for custom/local endpoints

# Agent B configuration (can differ from A)
agent_b_provider = "anthropic"
agent_b_model = "claude-sonnet-4-6"
agent_b_api_key = "sk-ant-..."        # ideally a DIFFERENT key for isolation
agent_b_base_url = ""

[isolation]
mode = "context"                      # context | process | container

[limits]
max_packages_per_job = 50
max_package_size_mb = 10
concurrency = 3                       # parallel package processing

[validation]
similarity_threshold = 0.70           # flag packages above this
run_tests = true                      # attempt to execute generated tests
syntax_check = true

[output]
default_license = "mit"
output_dir = "./phalus-output"
include_csp = true                    # bundle CSP spec in output
include_audit = true                  # bundle audit trail in output

[web]
enabled = false                       # set true to start local web UI
host = "127.0.0.1"
port = 3000

[doc_fetcher]
max_readme_size_kb = 500
max_type_def_size_kb = 200
max_code_example_lines = 10           # strip longer inline examples
github_token = ""                     # optional, for higher rate limits
```

### 6.2 Environment Variable Overrides

Every config key maps to an env var with `PHALUS_` prefix and double-underscore nesting:

```bash
PHALUS_LLM__AGENT_A_API_KEY=sk-ant-...
PHALUS_LLM__AGENT_B_API_KEY=sk-ant-...
PHALUS_LLM__AGENT_A_MODEL=claude-sonnet-4-6
PHALUS_ISOLATION__MODE=process
PHALUS_WEB__ENABLED=true
```

---

## 7. Supported Output Licenses

| License | ID | Notes |
|---------|----|-------|
| MIT | `mit` | Default. Permissive, minimal restrictions. |
| Apache 2.0 | `apache-2.0` | Permissive with patent grant |
| BSD 2-Clause | `bsd-2` | Permissive |
| BSD 3-Clause | `bsd-3` | Permissive |
| ISC | `isc` | Permissive (npm default) |
| Unlicense | `unlicense` | Public domain dedication |
| CC0 1.0 | `cc0` | Public domain dedication |
| Custom | `--license-file` | User provides license text via file |

---

## 8. Storage & Output Layout

Everything lives on the local filesystem. No database.

### 8.1 Global State

```
~/.phalus/
├── config.toml           # user configuration
├── cache/
│   └── csp/              # cached CSP specs by package@version hash
│       └── lodash@4.17.21-abc123.json
└── logs/
    └── phalus.log        # operational log
```

CSP caching: if you've already analyzed `lodash@4.17.21` and the docs haven't changed (by content hash), skip Agent A and reuse the cached spec. Agent B always runs fresh since the implementation should be independently generated each time.

### 8.2 Job Output

```
<output-dir>/
├── lodash/
│   ├── package.json
│   ├── LICENSE
│   ├── README.md
│   ├── src/
│   │   └── index.js
│   ├── test/
│   │   └── index.test.js
│   └── .cleanroom/
│       ├── csp/
│       │   ├── 01-overview.md
│       │   ├── 02-api-surface.json
│       │   ├── ...
│       │   └── 10-metadata.json
│       ├── audit.jsonl
│       └── similarity.json
├── express/
│   └── ...
├── .phalus/
│   ├── manifest.json     # original parsed manifest
│   ├── job.json           # job metadata (start time, config snapshot, completion)
│   └── audit.jsonl        # job-level audit trail (all packages)
└── README.md              # generated index of all reimplemented packages
```

---

## 9. Cross-Language Reimplementation

One of the strongest defenses against similarity/contamination claims is reimplementing in a different language. If the original is JavaScript and the output is Rust, structural similarity from LLM training data is dramatically reduced.

```bash
phalus run package.json --license apache-2.0 --target-lang rust
```

**How it works:**
- Agent A produces the same language-neutral CSP specification
- Agent B receives an additional instruction: "Implement in Rust (idiomatic, using standard library conventions)"
- Output is a Cargo project instead of an npm package
- API surface mapping is documented in the CSP (JS function → Rust function/method)

**Supported target languages (Phase 1):**
- Same language as original (default)
- Rust
- Go
- Python
- TypeScript (for JS originals that lack types)

This aligns with the observation from gwern on the Malus HN thread: an AI translating JS to Rust can't be copying GCC any more than Anthropic's Rust C compiler can — the structural divergence is inherent in the language change.

---

## 10. Project Structure

```
phalus/
├── README.md
├── LICENSE                          # 0BSD
├── Cargo.toml                       # if Rust, or package.json if Node
│
├── src/
│   ├── main.rs                      # CLI entry point
│   ├── config.rs                    # Config file + env var loading
│   │
│   ├── manifest/                    # Manifest parsing
│   │   ├── mod.rs
│   │   ├── npm.rs
│   │   ├── pypi.rs
│   │   ├── cargo.rs
│   │   ├── maven.rs
│   │   ├── go.rs
│   │   ├── rubygems.rs
│   │   └── composer.rs
│   │
│   ├── registry/                    # Registry metadata resolution
│   │   ├── mod.rs
│   │   ├── npm.rs
│   │   ├── pypi.rs
│   │   └── ...
│   │
│   ├── docs/                        # Documentation fetcher
│   │   ├── mod.rs
│   │   ├── github.rs
│   │   ├── type_defs.rs
│   │   ├── docs_site.rs
│   │   └── source_guard.rs          # HARD FILTER: blocks source code files
│   │
│   ├── agents/                      # LLM agent orchestration
│   │   ├── mod.rs
│   │   ├── analyzer.rs              # Agent A
│   │   ├── builder.rs               # Agent B
│   │   ├── prompts/
│   │   │   ├── analyzer_system.txt
│   │   │   └── builder_system.txt
│   │   └── providers/
│   │       ├── anthropic.rs
│   │       ├── openai.rs
│   │       └── ollama.rs
│   │
│   ├── firewall/                    # Isolation enforcement
│   │   ├── mod.rs
│   │   ├── context.rs               # Separate API calls
│   │   ├── process.rs               # Separate OS processes
│   │   └── container.rs             # Separate Docker containers
│   │
│   ├── validator/                   # Post-generation validation
│   │   ├── mod.rs
│   │   ├── syntax.rs
│   │   ├── tests.rs
│   │   ├── api_surface.rs
│   │   ├── license_check.rs
│   │   └── similarity.rs
│   │
│   ├── audit/                       # Audit trail
│   │   ├── mod.rs
│   │   └── logger.rs
│   │
│   ├── cache/                       # CSP caching
│   │   └── mod.rs
│   │
│   └── web/                         # Optional local web UI
│       ├── mod.rs
│       ├── routes.rs
│       └── static/                  # Embedded SPA assets
│           └── index.html
│
├── licenses/                        # Output license templates
│   ├── mit.txt
│   ├── apache-2.0.txt
│   ├── bsd-2.txt
│   ├── bsd-3.txt
│   ├── isc.txt
│   ├── unlicense.txt
│   └── cc0.txt
│
├── tests/
│   ├── manifest_parsing.rs
│   ├── registry_resolver.rs
│   ├── source_guard.rs
│   ├── firewall.rs
│   ├── e2e_left_pad.rs
│   └── e2e_is_odd.rs
│
└── docs/
    ├── architecture.md
    ├── legal-analysis.md
    └── configuration.md
```

**Why Rust:** Security-critical tool, needs to be fast, benefits from strong typing for the complex pipeline, and you already like Rust for this kind of thing. But the architecture is language-agnostic — a Node/TypeScript implementation would work fine for Phase 1 if speed-to-prototype matters more.

---

## 11. Testing Strategy

### 11.1 Unit Tests
- Manifest parsers: known-good manifests for each ecosystem
- Registry resolvers: mocked HTTP responses
- Source code guard: verify `.js`, `.py`, `.rs`, etc. are blocked; verify `.d.ts`, `README.md` are allowed
- Similarity scoring: known inputs with expected similarity values

### 11.2 Integration Tests
- End-to-end pipeline with `left-pad` (trivial package, fast round-trip)
- End-to-end with `is-odd` or `is-number` (tiny but real)
- Verify CSP spec completeness for well-documented packages
- Verify firewall: mock Agent B and confirm it never receives doc content
- Verify audit trail completeness — every required event present

### 11.3 Smoke Tests
- Clean room `chalk` and verify color output works
- Clean room a medium package, run generated tests, check pass rate
- Verify similarity stays below threshold on repeated runs
- Cross-language: clean room a JS package into Rust, verify it compiles

---

## 12. Roadmap

### Phase 1 — MVP (CLI only)
- npm/package.json support only
- Single LLM provider (Anthropic Claude)
- Context-level isolation
- CLI with `plan`, `run`, `run-one`, `inspect` commands
- Local filesystem storage + CSP caching
- Basic token-level similarity scoring
- MIT + Apache-2.0 license templates

### Phase 2 — Multi-Ecosystem + Web UI
- Add Python, Rust, Go parsers and resolvers
- Local web UI (single HTML + SSE)
- Cross-language reimplementation (`--target-lang`)
- AST-level similarity analysis
- Process-level isolation option
- All license templates

### Phase 3 — Hardening
- Container-level isolation
- OpenAI + Ollama provider support (local LLMs)
- Transitive dependency resolution
- Automated test execution in sandboxed environments
- SBOM integration (SPDX, CycloneDX output)
- Batch operations for large manifests

### Phase 4 — Advanced
- Multiple generation passes with divergence selection
- Integration with existing SBOM/SCA tooling
- GitHub Action / CI integration
- Cryptographic attestation of audit trail (potential AgentPin/SchemaPin integration point)
- Plugin system for custom ecosystems

---

## 13. Legal & Compliance Considerations

### 13.1 What Makes This a Clean Room

Three pillars:

1. **Separation of knowledge**: Agent A reads docs; Agent B reads specs. Agent B never sees source code or original documentation.
2. **Independent creation**: Agent B's output is a fresh implementation guided only by a functional specification.
3. **Audit trail**: Every step is logged with SHA-256 checksums, providing evidence of the process if challenged.

### 13.2 Known Legal Risks & Open Questions

- **LLM training data contamination**: If the LLM trained on the original source code, its output may reproduce patterns, variable names, or algorithmic structure from training data. This is the strongest argument against AI clean rooms. The Malus HN discussion specifically highlights this: "the contamination happens at the training phase, not the inference phase."
- **Thin copyright**: Simple utility functions may lack enough creative expression to be copyrightable, making clean room unnecessary but also infringement hard to prove.
- **API copyrightability**: *Oracle v. Google* (2021) found API reimplementation can be fair use, but boundaries are still being tested.
- **Jurisdiction variance**: Copyright law varies. Clean room methodology is strongest under U.S. law.

### 13.3 Mitigation Strategies

- **Similarity scoring**: Flag and review any output with high similarity
- **Target language switching**: Reimplement in a different language to force structural divergence
- **Multiple generation passes**: Generate several implementations, select the most divergent
- **Human review**: For high-risk packages (AGPL, large codebases), review output manually
- **Local/fine-tuned models**: Use models NOT trained on the specific codebase (strongest isolation, hardest to achieve)

---

## 14. References

- [Malus — Clean Room as a Service](https://malus.sh/)
- [Malus Blog — "Thank You for Your Service"](https://malus.sh/blog.html)
- [FOSDEM 2026 — "Let's end open source together with this one simple trick"](https://fosdem.org/2026/schedule/event/SUVS7G-lets_end_open_source_together_with_this_one_simple_trick/) — Dylan Ayrey & Mike Nolan
- *Baker v. Selden*, 101 U.S. 99 (1879) — copyright protects expression, not ideas
- *Google LLC v. Oracle America, Inc.*, 593 U.S. 1 (2021) — API reimplementation as fair use
- [Phoenix Technologies BIOS clean room](https://en.wikipedia.org/wiki/Phoenix_Technologies#Cloning_the_IBM_PC_BIOS)
- [chardet relicensing controversy (March 2026)](https://gigazine.net/gsc_news/en/20260313-malus-open-source/) — real-world AI-assisted relicensing dispute
