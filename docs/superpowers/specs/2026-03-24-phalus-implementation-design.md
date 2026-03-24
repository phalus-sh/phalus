# PHALUS Implementation Design

**Date:** 2026-03-24
**Scope:** Phase 1 (MVP, npm-only CLI) + Phase 2 (multi-ecosystem, web UI)
**Language:** Rust
**License:** 0BSD

---

## 1. Core Architecture

Sequential pipeline of typed stages:

```
ManifestParser → RegistryResolver → DocFetcher → Analyzer → Firewall → Builder → Validator
```

Each stage takes typed input and produces typed output. The `Pipeline` struct orchestrates them, driving packages in parallel via `tokio::JoinSet` up to a configurable concurrency limit.

### Key Types

- `ParsedManifest` — output of manifest parsing
- `PackageMetadata` — enriched by registry resolver
- `Documentation` — fetched docs (source guard enforced)
- `CspSpec` — the 10-document specification pack
- `Implementation` — generated source code + tests
- `ValidationReport` — similarity scores, test results, pass/fail
- `AuditEvent` — enum of all audit event types, serialized to JSONL

### Error Handling

`thiserror` for typed errors per component, `anyhow` at the CLI boundary. Pipeline errors are per-package — one failure doesn't stop the others.

### Concurrency

`tokio` runtime. Packages processed concurrently up to configured limit. Each package's pipeline stages run sequentially.

---

## 2. Manifest Parsing & Registry Resolution

### Manifest Parsing

Trait-based with per-ecosystem implementations:

```rust
trait ManifestParser {
    fn detect(path: &Path) -> bool;
    fn parse(content: &str) -> Result<ParsedManifest>;
}
```

Auto-detection by filename. Phase 1: `package.json`. Phase 2 adds: `requirements.txt`, `pyproject.toml`, `Cargo.toml`, `go.mod`.

### Registry Resolution

`RegistryResolver` trait with per-ecosystem implementations. Each resolver:
1. Resolves version constraints to a concrete version
2. Fetches metadata (description, license, repo URL, size)
3. Respects `max_package_size_mb` limit (reject early, log to audit)

Phase 1: npm (`registry.npmjs.org`). Phase 2 adds: PyPI, crates.io, proxy.golang.org.

No transitive dependency resolution until Phase 3.

---

## 3. Documentation Fetcher & Source Guard

### Source Guard (hard filter, not configurable)

Blocked extensions: `.js`, `.py`, `.rs`, `.go`, `.java`, `.rb`, `.php`, `.c`, `.cpp`, `.cc`, `.h`, `.cs`, `.ts` (but NOT `.d.ts`). Blocked path patterns: `test/`, `tests/`, `__tests__/`, `spec/`. Any match is rejected and logged as `source_code_blocked` audit event.

### Doc Fetching Pipeline (per package)

1. Fetch `README.md` from GitHub API (`/repos/{owner}/{repo}/readme`)
2. For npm: extract `.d.ts` files from tarball (or DefinitelyTyped)
3. Fetch homepage/docs URL if available (HTML → strip to text)
4. Strip inline code examples longer than `max_code_example_lines` (default 10)
5. Collect package metadata from registry as structured text

Output: `Documentation` struct with named text blobs and content hashes.

### Doc Site Fetcher

Homepage/docs URL fetching is handled by `docs_site.rs` — fetches HTML pages, strips to text content, respects `max_readme_size_kb` limit. Separate from GitHub API fetching in `github.rs`.

---

## 4. Agents via Symbiont

Agent orchestration uses `symbi` (https://github.com/thirdkeyai/symbiont) instead of custom LLM clients.

### Two Agents in Symbi DSL

- **`analyzer`** (Agent A) — Capabilities: `[Network]`. Policy denies source code access. Receives `Documentation`, produces `CspSpec`.
- **`builder`** (Agent B) — Capabilities: `[FileSystem]`. Policy denies access to anything except CSP spec input. Receives `CspSpec`, produces `Implementation`.

### Why Symbiont

1. **Isolation for free** — Sandbox tiers map to spec's isolation modes: `context` → separate agent invocations, `process` → native execution isolation, `container` → Docker sandbox.
2. **Provider-agnostic** — Anthropic, OpenAI, Ollama supported via env vars. Phase 2 multi-provider comes free.
3. **Audit trail** — Durable journal + AgentPin give cryptographic audit of agent actions.
4. **Cedar policies** — Formally define what each agent can/cannot access. Firewall becomes a verifiable policy.

### What We Still Build

Manifest parsing, registry resolution, doc fetching, source guard, similarity scoring, CLI, web UI. Symbiont handles agent orchestration and isolation.

### Token Management

Agent A: 8k output default, Agent B: 16k output default (configurable). For large packages, Agent B may need multiple calls — chunk the API surface and stitch output.

---

## 5. Validation & Similarity Scoring

Five post-generation checks:

1. **Syntax check** — Shell out to target language parser/compiler (`node --check`, `cargo check -j2`, `python -m py_compile`, `go build`).
2. **Test execution** — Run generated tests in sandboxed environment. Capture pass/fail counts.
3. **API surface check** — Verify generated code exports every function/class/method from `02-api-surface.json`. Best-effort, language-specific.
4. **License check** — Verify license text in `LICENSE` and headers where convention requires (Apache-2.0).
5. **Similarity scoring** — Compare against original source (fetched ONLY by validator, AFTER Agent B finishes, NEVER shown to agents):
   - Token-level Jaccard similarity
   - Function-name overlap ratio (expected high, noted in report)
   - String/comment literal overlap
   - AST structural similarity (Phase 2, tree-sitter based)
   - Weighted overall score, threshold check (default 0.70), PASS/FAIL verdict

Original source fetch is logged as a separate audit event.

---

## 6. Audit Trail

### Two Levels, One User-Facing Trail

1. **Symbiont journal** — ORGA cycle events, agent invocations, policy decisions, sandbox state. Cryptographic audit via AgentPin + durable journal. Stored alongside but not inside the PHALUS audit.
2. **PHALUS pipeline audit** — JSONL log for pipeline stages: manifest parsing, doc fetching, source guard rejections, firewall crossings, validation results, job completion. This is the primary user-facing audit trail.

The PHALUS audit log references the symbiont journal by hash at the `spec_generated` and `implementation_generated` events. When a user runs `phalus inspect --audit`, they see the PHALUS trail. The symbiont journal is available as a supplementary artifact in `.cleanroom/symbiont-journal/` for deeper forensics.

### Every Audit Entry Includes

- ISO 8601 timestamp
- Monotonic sequence number
- Event type (enum → serde tag)
- SHA-256 of all inputs/outputs

### Tamper Detection

On job completion, hash entire `audit.jsonl` and write digest to `job.json`.

### CSP Caching

Cache key: `{package_name}@{version}-{content_hash}` where `content_hash` is SHA-256 of the fetched documentation content. If docs haven't changed, reuse the cached CSP. Cache hits logged as `spec_cache_hit` with original spec hashes. Agent B always runs fresh.

---

## 7. CLI & Web UI

### CLI

`clap` derive API. Binary: `phalus`. Commands:

- **`plan <manifest>`** — Parse manifest, display packages with name/version/size/license. No network calls beyond registry metadata.
- **`run <manifest>`** — Full pipeline. Options: `--license`, `--output`, `--only`, `--exclude`, `--target-lang`, `--isolation`, `--similarity-threshold`, `--concurrency`, `--dry-run`, `--verbose`.
- **`run-one <ecosystem/pkg@version>`** — Single-package shortcut without a manifest file.
- **`inspect <output-dir>`** — View completed job: `--audit` (full trail), `--similarity` (scores), `--csp` (spec summary).
- **`validate <output-dir>`** — Re-run validation on existing output without regeneration. Fetches original source for similarity re-scoring.
- **`config`** — Display resolved configuration (file + env vars + defaults).

`--dry-run` runs Agent A only, produces CSP specs without invoking Agent B.

Progress rendering via `indicatif` — each package gets a progress bar showing: fetching → analyzing → firewall → implementing → validating → done.

### Web UI (Phase 2)

`axum` server on `127.0.0.1:3000` (symbiont already uses axum — no new dependency).

- Single `index.html` with embedded JS/CSS via `rust-embed` or `include_str!`
- SSE for progress: `GET /api/jobs/:id/stream` via `tokio::broadcast`
- API endpoints match spec exactly
- No auth — localhost only, warning if bound to `0.0.0.0`

### Shared Core

Both CLI and web UI call `Pipeline::run()`. CLI renders to stderr, web UI sends over SSE.

---

## 8. Cross-Language Reimplementation (Phase 2)

The `--target-lang` flag changes Agent B's behavior without affecting Agent A.

**How it flows through the pipeline:**
1. Agent A produces the same language-neutral CSP specification regardless of target
2. The firewall passes the CSP plus the target language as metadata
3. Agent B receives an additional system prompt instruction: "Implement in {language} using idiomatic conventions and standard library"
4. Output project structure changes per language (Cargo project for Rust, Go module for Go, etc.)

**Supported targets (Phase 2):** same language as original (default), Rust, Go, Python, TypeScript.

**Java/Kotlin, Ruby, PHP, .NET** ecosystems and target languages are deferred to Phase 3+.

---

## 9. Project Structure

```
phalus/
├── Cargo.toml
├── LICENSE
├── README.md
├── .gitignore
│
├── agents/                          # Symbi DSL agent definitions
│   ├── analyzer.dsl
│   └── builder.dsl
│
├── src/
│   ├── main.rs                      # CLI entry point (clap)
│   ├── lib.rs                       # Public API for shared core
│   ├── config.rs                    # Config file + env var loading
│   ├── pipeline.rs                  # Orchestrator
│   │
│   ├── manifest/                    # Manifest parsing
│   │   ├── mod.rs
│   │   ├── npm.rs
│   │   ├── pypi.rs
│   │   ├── cargo.rs
│   │   └── gomod.rs
│   │
│   ├── registry/                    # Registry metadata resolution
│   │   ├── mod.rs
│   │   ├── npm.rs
│   │   ├── pypi.rs
│   │   ├── crates.rs
│   │   └── golang.rs
│   │
│   ├── docs/                        # Documentation fetcher
│   │   ├── mod.rs
│   │   ├── github.rs
│   │   ├── type_defs.rs
│   │   ├── docs_site.rs
│   │   └── source_guard.rs
│   │
│   ├── agents/                      # Agent orchestration via symbiont
│   │   ├── mod.rs
│   │   ├── analyzer.rs
│   │   ├── builder.rs
│   │   └── prompts/
│   │       ├── analyzer_system.txt
│   │       └── builder_system.txt
│   │
│   ├── firewall/                    # Delegates to symbiont sandbox
│   │   └── mod.rs
│   │
│   ├── validator/                   # Post-generation validation
│   │   ├── mod.rs
│   │   ├── syntax.rs
│   │   ├── tests.rs
│   │   ├── api_surface.rs
│   │   ├── license_check.rs
│   │   └── similarity.rs
│   │
│   ├── audit/                       # Pipeline-level audit trail
│   │   └── mod.rs
│   │
│   ├── cache/                       # CSP caching
│   │   └── mod.rs
│   │
│   └── web/                         # Local web UI (Phase 2)
│       ├── mod.rs
│       ├── routes.rs
│       └── static/
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
└── tests/
    ├── manifest_parsing.rs
    ├── registry_resolver.rs
    ├── source_guard.rs
    ├── firewall.rs
    ├── e2e_left_pad.rs
    └── e2e_is_odd.rs
```

---

## 10. Key Crate Dependencies

| Crate | Purpose |
|-------|---------|
| `symbi` | Agent runtime, isolation, LLM providers |
| `clap` | CLI parsing (derive) |
| `tokio` | Async runtime |
| `reqwest` | HTTP client (registry, GitHub API, doc fetching) |
| `axum` | Web UI server (Phase 2, already a symbi dep) |
| `serde` / `serde_json` | Serialization |
| `toml` | Config file parsing |
| `sha2` | SHA-256 checksums for audit |
| `thiserror` | Typed errors per component |
| `anyhow` | Error handling at CLI boundary |
| `indicatif` | CLI progress bars |
| `rust-embed` | Embed static web assets (Phase 2) |
| `chrono` | Timestamps for audit entries |

---

## 11. Decisions & Trade-offs

| Decision | Rationale |
|----------|-----------|
| Symbiont for agents | Built-in isolation, policies, audit — avoids reimplementing what already exists |
| Thin API clients dropped | Symbiont handles provider abstraction |
| No database | Filesystem-only, per spec. JSONL for audit, TOML for config, JSON for structured data |
| `rust-embed` for web UI | No build step, no node_modules, single binary distribution |
| Source guard as hard filter | Not configurable — the clean room claim depends on it |
| Similarity scoring fetches source post-build only | Validator-only, never enters agent pipeline |
