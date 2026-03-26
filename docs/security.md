# Security

This page describes the security features built into PHALUS, their limitations, and guidance for operating the tool safely.

---

## Source Guard (Hard Filter)

The most important security and integrity property of PHALUS is that Agent A never receives source code. The source guard is implemented as an unconditional hard filter in the documentation fetcher.

Any file with a source code extension is rejected before it can be included in the documentation bundle passed to Agent A:

- Blocked extensions: `.js`, `.ts`, `.jsx`, `.tsx`, `.py`, `.rs`, `.go`, `.java`, `.kt`, `.rb`, `.php`, `.c`, `.cpp`, `.h`, `.cs`, `.swift`, and others
- Allowed: `.md`, `.rst`, `.txt`, `.d.ts`, `.html`, `.json` (for metadata)

The filter is not configurable. Removing or weakening it would invalidate the clean room claim and is not supported.

Every blocked file is recorded as a `source_code_blocked` audit event with the file path and reason. The absence of such events in the audit log for a normal run confirms that no source code was encountered.

---

## Path Traversal Protection

When writing generated files to the output directory, PHALUS validates that every file path stays within the package's output directory. Two checks are applied:

1. **String check**: Any filename containing `..` is rejected before a path is constructed.
2. **Canonical path check**: The resolved (canonicalised) path of the target file is checked to confirm it starts with the resolved base directory. If not, the write is aborted with a `PermissionDenied` error.

This protects against a malicious or misbehaving LLM response that includes filenames like `../../etc/passwd` or `../../../home/user/.ssh/authorized_keys`.

The same checks are applied to both the implementation files written by Agent B and the CSP document files written during the firewall stage.

---

## API Key Redaction

API keys are never written to the output directory, the audit log, or any log output.

- `phalus config` redacts all API keys in its output, showing `***` in place of any non-empty key.
- The `LlmConfig` and `DocFetcherConfig` types implement a custom `Debug` formatter that redacts key values.
- No API key or credential value is included in the audit log.

If you suspect an API key has been exposed (for example, via an error message or log output), rotate the key immediately at your provider's console.

---

## XSS Protection in Web UI

The web UI is a single-page application served from an embedded static HTML file. It does not server-render user-supplied data directly into HTML. Package names, file contents, and audit log entries displayed in the UI are inserted into the DOM via JavaScript text nodes or JSON, not via `innerHTML`, to prevent cross-site scripting if malicious content appears in generated output or package metadata.

The web UI has no authentication layer. If you expose the server to a network, anyone who can reach it can read all output and trigger new pipeline runs. Do not expose the web UI to untrusted networks.

---

## Non-Root Docker Container

The official Docker image runs as a non-root user (`phalus`, UID 1000). This limits the damage from a container escape or a vulnerability in the PHALUS binary itself — the process cannot write to system directories or read files owned by other users on the host.

See [Docker](docker.md) for guidance on file ownership when using volume mounts.

---

## Audit Trail Integrity

The audit log is append-only during a run. When the job completes, `finalize()` computes a SHA-256 hash of the entire log file and records it in the `job_completed` event. This hash can be recomputed at any time to detect post-completion modifications.

PHALUS does not use a cryptographically linked ledger or external timestamping service in the current implementation. The audit trail is therefore self-asserted: it proves the sequence of events as recorded by PHALUS, but does not provide an externally verifiable timestamp. Future versions may integrate with an external attestation mechanism.

---

## Network Access

PHALUS makes outbound HTTPS requests to:

- Package registry APIs (npm, PyPI, crates.io, proxy.golang.org)
- GitHub API (`api.github.com`) for README and type definition fetching
- Documentation sites linked from package metadata
- LLM provider APIs (Anthropic, OpenAI, or a configured custom endpoint)

It does not make any other outbound connections. No telemetry, no usage reporting, no update checks.

If you want to restrict network access for the container isolation mode, use Docker network policies to allow only the required endpoints.

---

## Known Limitations

### LLM Training Data Contamination

This is the strongest argument against AI-assisted clean room reimplementation. If the LLM used for Agent A or Agent B was trained on the original package's source code, its output may reproduce variable names, algorithmic structure, or even code patterns from training data — regardless of whether the agent was shown that source code during inference.

PHALUS mitigates this risk through:

- **Similarity scoring**: Generated output is compared against the original source. High similarity triggers a FAIL verdict, prompting manual review.
- **Cross-language reimplementation**: `--target-lang` forces structural divergence. A JavaScript package reimplemented in Rust cannot reproduce JavaScript idioms verbatim.
- **Threshold configuration**: Lower the `similarity_threshold` below the default 0.70 for higher-risk packages.

However, PHALUS cannot fully eliminate this risk. The training data contamination issue is inherent to any LLM-based clean room approach. For packages where the legal risk is high, human review of the generated output is strongly recommended. Consider using locally-hosted fine-tuned models that were not trained on the target codebase.

### Thin Copyright and API Surface

Simple utility functions with minimal creative expression may not be copyrightable in the first place. PHALUS does not assess copyrightability — it runs the pipeline regardless. High similarity scores on trivially simple packages may reflect inherent algorithmic convergence, not copying.

### Jurisdiction Variance

Clean room methodology has the strongest legal foundation under U.S. copyright law. Its standing in other jurisdictions varies. Consult legal counsel before relying on PHALUS output in a commercial context.

---

## Reporting Vulnerabilities

Security issues can be reported by opening a private security advisory on the GitHub repository. Do not file public issues for security-sensitive findings.
