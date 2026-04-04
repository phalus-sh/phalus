# Audit Trail

The audit trail is the primary legal evidence that a PHALUS run followed the clean room methodology. Every pipeline stage logs a structured record before passing data to the next stage. The complete trail proves, in order: what documentation was read, that source code was never accessed by the agents, what specification was produced and by which model, what crossed the firewall and with what checksums, what implementation was produced, and what the validation verdict was.

---

## Format

Audit records are written to `<output-dir>/audit.jsonl` in JSON Lines format — one JSON object per line, appended in order.

Each entry has three top-level fields:

| Field | Type | Description |
|-------|------|-------------|
| `timestamp` | string | RFC 3339 UTC timestamp |
| `seq` | integer | Monotonically increasing sequence number starting at 0 |
| `event` | object | Event-specific payload; always includes a `type` field |

The `type` field uses snake_case and identifies one of the ten event types listed below.

---

## Event Types

### `manifest_parsed`

Emitted once per job when the manifest file is successfully parsed.

```json
{
  "timestamp": "2026-03-26T10:00:00Z",
  "seq": 0,
  "event": {
    "type": "manifest_parsed",
    "manifest_hash": "a3f2c1d4e5b6...",
    "package_count": 5
  }
}
```

| Field | Description |
|-------|-------------|
| `manifest_hash` | SHA-256 hex digest of the manifest file contents |
| `package_count` | Number of packages after filtering |

---

### `docs_fetched`

Emitted for each package after documentation is retrieved. Records every URL accessed and a content hash for each document.

```json
{
  "timestamp": "2026-03-26T10:00:01Z",
  "seq": 1,
  "event": {
    "type": "docs_fetched",
    "package": "lodash@4.17.21",
    "urls_accessed": [
      "https://api.github.com/repos/lodash/lodash/readme",
      "https://lodash.com/docs/4.17.15"
    ],
    "content_hashes": {
      "readme": "b4c1a2d3...",
      "docs_site": "e5f6a7b8..."
    }
  }
}
```

---

### `source_code_blocked`

Emitted if the source guard encounters and rejects a source code file during documentation fetching. This event should never appear in a normal run that does not attempt to access source files. Its presence indicates that a source file was encountered at a URL that was being fetched for documentation purposes.

```json
{
  "timestamp": "2026-03-26T10:00:01Z",
  "seq": 2,
  "event": {
    "type": "source_code_blocked",
    "package": "lodash@4.17.21",
    "path": "https://raw.githubusercontent.com/lodash/lodash/main/lodash.js",
    "reason": "source file extension blocked: .js"
  }
}
```

---

### `spec_generated`

Emitted after Agent A produces the CSP specification. Records a content hash for each of the ten CSP documents, the model used, and a hash of the prompt sent to the model.

```json
{
  "timestamp": "2026-03-26T10:00:03Z",
  "seq": 3,
  "event": {
    "type": "spec_generated",
    "package": "lodash@4.17.21",
    "document_hashes": {
      "01-overview.json": "c1d2e3f4...",
      "02-api-surface.json": "a5b6c7d8...",
      "03-behavior-spec.json": "e9f0a1b2..."
    },
    "model": "claude-sonnet-4-6",
    "prompt_hash": "f3a4b5c6...",
    "symbiont_journal_hash": null
  }
}
```

| Field | Description |
|-------|-------------|
| `document_hashes` | SHA-256 hex digest of each CSP document content |
| `model` | Model identifier used for the API call |
| `prompt_hash` | SHA-256 hex digest of the full prompt sent to Agent A |
| `symbiont_journal_hash` | Reserved for future Symbiont Journal integration; `null` currently |

---

### `spec_cache_hit`

Emitted instead of `spec_generated` when a cached CSP is used because the package, version, and documentation content hash all match a previous run.

```json
{
  "timestamp": "2026-03-26T10:00:03Z",
  "seq": 3,
  "event": {
    "type": "spec_cache_hit",
    "package": "lodash@4.17.21",
    "spec_hashes": {
      "01-overview.json": "c1d2e3f4...",
      "02-api-surface.json": "a5b6c7d8..."
    }
  }
}
```

---

### `firewall_crossing`

Emitted when the CSP documents are handed from Agent A to Agent B. This is the critical boundary event. It explicitly records that only specification documents crossed, with their checksums, and asserts that source code was not accessed.

```json
{
  "timestamp": "2026-03-26T10:00:03Z",
  "seq": 4,
  "event": {
    "type": "firewall_crossing",
    "package": "lodash@4.17.21",
    "documents_transferred": [
      "01-overview.json",
      "02-api-surface.json",
      "03-behavior-spec.json",
      "04-edge-cases.json",
      "05-configuration.json",
      "06-type-definitions.json",
      "07-error-catalog.json",
      "08-compatibility-notes.json",
      "09-test-scenarios.json",
      "10-metadata.json"
    ],
    "sha256_checksums": {
      "01-overview.json": "c1d2e3f4...",
      "02-api-surface.json": "a5b6c7d8..."
    },
    "isolation_mode": "context",
    "source_code_accessed": false
  }
}
```

---

### `implementation_generated`

Emitted after Agent B produces the implementation. Records a SHA-256 hash for each generated file and the prompt hash.

```json
{
  "timestamp": "2026-03-26T10:00:08Z",
  "seq": 5,
  "event": {
    "type": "implementation_generated",
    "package": "lodash@4.17.21",
    "file_hashes": {
      "src/index.js": "d1e2f3a4...",
      "package.json": "b5c6d7e8...",
      "LICENSE": "f9a0b1c2..."
    },
    "model": "claude-sonnet-4-6",
    "prompt_hash": "a3b4c5d6...",
    "symbiont_journal_hash": null
  }
}
```

---

### `original_source_fetched`

Emitted when the validator fetches the original package source for similarity comparison. This event documents that source code was accessed by the validator only, not by either agent.

```json
{
  "timestamp": "2026-03-26T10:00:09Z",
  "seq": 6,
  "event": {
    "type": "original_source_fetched",
    "package": "lodash@4.17.21",
    "source_length": 17408,
    "fetched": true
  }
}
```

---

### `validation_completed`

Emitted after all validation checks complete for a package.

```json
{
  "timestamp": "2026-03-26T10:00:09Z",
  "seq": 7,
  "event": {
    "type": "validation_completed",
    "package": "lodash@4.17.21",
    "syntax_ok": true,
    "tests_passed": 12,
    "tests_failed": 0,
    "similarity_score": 0.2800,
    "verdict": "pass"
  }
}
```

---

### `job_completed`

Emitted once at the end of a job, after all packages have been processed. Contains the SHA-256 hash of the entire audit log file up to this point, providing tamper detection.

```json
{
  "timestamp": "2026-03-26T10:05:00Z",
  "seq": 42,
  "event": {
    "type": "job_completed",
    "packages_processed": 5,
    "packages_failed": 0,
    "total_elapsed_secs": 287.4,
    "audit_log_hash": "8f3a2b1c4d5e6f7a..."
  }
}
```

---

## Tamper Detection

When a job completes, `AuditLogger::finalize()` reads the entire audit file, computes a SHA-256 hash of its contents, and returns the hex digest. This hash is then written as the `audit_log_hash` field in the `job_completed` event.

To verify integrity after the fact:

```bash
# Remove the last line (job_completed), hash the rest
head -n -1 ./phalus-output/audit.jsonl | sha256sum
```

Compare the output to the `audit_log_hash` in the `job_completed` event. A mismatch indicates the log was modified after completion.

Note: The `job_completed` event itself is appended after the hash is computed, so the hash covers all preceding events only.

---

## Symbiont Journal Integration

The `symbiont_journal_hash` field in `spec_generated` and `implementation_generated` events is reserved for future integration with the Symbiont Journal concept — a cryptographically-linked ledger of LLM inference sessions. It is `null` in all current builds. When implemented, it would provide an externally verifiable timestamp and proof that a specific model inference occurred, strengthening the audit trail beyond what PHALUS alone can assert.

---

## Inspecting the Audit Log

Use the CLI:

```bash
phalus inspect ./phalus-output --audit
```

Or read the file directly:

```bash
# Pretty-print all entries
jq '.' ./phalus-output/audit.jsonl

# Show only firewall crossings
jq 'select(.event.type == "firewall_crossing")' ./phalus-output/audit.jsonl

# Show verdict for each package
jq 'select(.event.type == "validation_completed") | {pkg: .event.package, verdict: .event.verdict}' \
  ./phalus-output/audit.jsonl
```
