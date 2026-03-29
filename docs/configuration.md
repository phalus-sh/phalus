# Configuration

PHALUS is configured via a TOML file and environment variable overrides. CLI flags override both for per-run settings.

---

## Config File Location

```
~/.phalus/config.toml
```

If the file does not exist, all defaults apply. Create the directory and file manually:

```bash
mkdir -p ~/.phalus
touch ~/.phalus/config.toml
```

Run `phalus config` to see the active configuration (API keys redacted).

---

## Complete Reference

```toml
# ~/.phalus/config.toml

[llm]
# Provider for Agent A (Analyzer). Supported: anthropic, openai, ollama
agent_a_provider = "anthropic"

# Model identifier sent to the provider API
agent_a_model = "claude-sonnet-4-6"

# API key for Agent A. Required — no default.
agent_a_api_key = ""

# Optional base URL override for Agent A. Use this for local/proxy endpoints.
# Leave empty to use the provider's default endpoint.
agent_a_base_url = ""

# Provider for Agent B (Builder). Can differ from Agent A.
agent_b_provider = "anthropic"

# Model identifier for Agent B
agent_b_model = "claude-sonnet-4-6"

# API key for Agent B. Ideally a separate key from Agent A for stronger isolation evidence.
# Required — no default.
agent_b_api_key = ""

# Optional base URL override for Agent B.
agent_b_base_url = ""

[llm.retry]
# Maximum number of retry attempts (not counting the initial request).
max_retries = 3

# Initial backoff in milliseconds; doubles on each retry.
initial_backoff_ms = 500

# Per-request timeout in seconds.
timeout_secs = 120


[isolation]
# Isolation strategy between Agent A and Agent B.
# context   — Separate API calls with independent conversation contexts. Default.
# process   — Separate OS processes with no shared memory.
# container — Separate Docker containers with no network overlap.
mode = "context"

# Docker image for container isolation mode
docker_image = "alpine:3"

# Memory limit for the isolation container
memory_limit = "256m"

# CPU limit for the isolation container
cpu_limit = "1.0"

# Seconds before the container run is killed
timeout_secs = 60

# Docker network mode ("none" for full isolation)
network_mode = "none"

# Maximum PIDs inside the isolation container
pids_limit = 64


[limits]
# Maximum number of packages allowed in a single job.
max_packages_per_job = 50

# Maximum unpacked size of a single package in megabytes.
# Packages exceeding this are skipped.
max_package_size_mb = 10

# Number of packages to process in parallel.
concurrency = 3


[validation]
# Similarity score threshold (0.0–1.0). Packages with an overall_score above
# this value receive a FAIL verdict. Default 0.70.
similarity_threshold = 0.70

# Whether to execute the generated tests after Agent B produces output.
# Requires the relevant language runtime to be installed.
run_tests = true

# Whether to run a syntax check on generated code.
# Requires the relevant language toolchain.
syntax_check = true


[output]
# Default SPDX license identifier applied to generated code.
# Valid values: mit, apache-2.0, bsd-2, bsd-3, isc, unlicense, cc0
default_license = "mit"

# Default output directory for generated packages.
output_dir = "./phalus-output"

# Whether to include the CSP specification documents in the output directory.
include_csp = true

# Whether to include the audit trail in the output directory.
include_audit = true


[web]
# Set to true to start the local web UI automatically on launch.
enabled = false

# Address to bind to. Use 127.0.0.1 (default) to restrict to local access.
host = "127.0.0.1"

# Port for the web UI.
port = 3000


[doc_fetcher]
# Maximum size of a fetched README in kilobytes.
max_readme_size_kb = 500

# Maximum size of fetched type definition files in kilobytes.
max_type_def_size_kb = 200

# Inline code examples longer than this many lines are stripped before
# being passed to Agent A.
max_code_example_lines = 10

# Optional GitHub personal access token. Used to increase the GitHub API
# rate limit when fetching READMEs and type definitions. Leave empty to
# use unauthenticated requests (60 requests/hour per IP).
github_token = ""
```

---

## Sections

### `[llm]`

Controls the LLM provider, model, and API credentials for each agent. Agent A and Agent B can use different providers and models. Using different API keys is recommended when presenting the audit trail as legal evidence of isolation.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `agent_a_provider` | string | `anthropic` | Provider identifier |
| `agent_a_model` | string | `claude-sonnet-4-6` | Model identifier |
| `agent_a_api_key` | string | `""` | API key (required) |
| `agent_a_base_url` | string | `""` | Custom base URL (optional) |
| `agent_b_provider` | string | `anthropic` | Provider identifier |
| `agent_b_model` | string | `claude-sonnet-4-6` | Model identifier |
| `agent_b_api_key` | string | `""` | API key (required) |
| `agent_b_base_url` | string | `""` | Custom base URL (optional) |
| `retry.max_retries` | integer | `3` | Max retry attempts per LLM request |
| `retry.initial_backoff_ms` | integer | `500` | Initial backoff (doubles each retry) |
| `retry.timeout_secs` | integer | `120` | Per-request timeout in seconds |

### `[isolation]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `mode` | string | `context` | `context`, `process`, or `container` |
| `docker_image` | string | `alpine:3` | Docker image for container mode |
| `memory_limit` | string | `256m` | Container memory limit |
| `cpu_limit` | string | `1.0` | Container CPU limit |
| `timeout_secs` | integer | `60` | Container timeout in seconds |
| `network_mode` | string | `none` | Docker network mode |
| `pids_limit` | integer | `64` | Max PIDs in container |

### `[limits]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `max_packages_per_job` | integer | `50` | Package cap per job |
| `max_package_size_mb` | integer | `10` | Per-package size cap in MB |
| `concurrency` | integer | `3` | Parallel package processing |

### `[validation]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `similarity_threshold` | float | `0.70` | Pass/fail cutoff |
| `run_tests` | bool | `true` | Execute generated tests |
| `syntax_check` | bool | `true` | Run syntax validation |

### `[output]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `default_license` | string | `mit` | SPDX license for generated code |
| `output_dir` | string | `./phalus-output` | Output root |
| `include_csp` | bool | `true` | Bundle CSP in output |
| `include_audit` | bool | `true` | Bundle audit log in output |

### `[web]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Auto-start web UI |
| `host` | string | `127.0.0.1` | Bind address |
| `port` | integer | `3000` | Listen port |

### `[doc_fetcher]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `max_readme_size_kb` | integer | `500` | README size cap |
| `max_type_def_size_kb` | integer | `200` | Type definition size cap |
| `max_code_example_lines` | integer | `10` | Code block stripping threshold |
| `github_token` | string | `""` | GitHub personal access token |

---

## Environment Variable Overrides

Every config key can be overridden via an environment variable. Variables use the `PHALUS_` prefix followed by the TOML section in uppercase, a double underscore (`__`), and the key name in uppercase.

Pattern: `PHALUS_<SECTION>__<KEY>`

| Environment Variable | Config key |
|----------------------|------------|
| `PHALUS_LLM__AGENT_A_API_KEY` | `llm.agent_a_api_key` |
| `PHALUS_LLM__AGENT_B_API_KEY` | `llm.agent_b_api_key` |
| `PHALUS_LLM__AGENT_A_MODEL` | `llm.agent_a_model` |
| `PHALUS_LLM__AGENT_B_MODEL` | `llm.agent_b_model` |
| `PHALUS_LLM__AGENT_A_PROVIDER` | `llm.agent_a_provider` |
| `PHALUS_LLM__AGENT_A_BASE_URL` | `llm.agent_a_base_url` |
| `PHALUS_LLM__AGENT_B_BASE_URL` | `llm.agent_b_base_url` |
| `PHALUS_LLM__RETRY_MAX_RETRIES` | `llm.retry.max_retries` |
| `PHALUS_LLM__RETRY_INITIAL_BACKOFF_MS` | `llm.retry.initial_backoff_ms` |
| `PHALUS_LLM__RETRY_TIMEOUT_SECS` | `llm.retry.timeout_secs` |
| `PHALUS_ISOLATION__MODE` | `isolation.mode` |
| `PHALUS_ISOLATION__DOCKER_IMAGE` | `isolation.docker_image` |
| `PHALUS_ISOLATION__MEMORY_LIMIT` | `isolation.memory_limit` |
| `PHALUS_ISOLATION__CPU_LIMIT` | `isolation.cpu_limit` |
| `PHALUS_ISOLATION__TIMEOUT_SECS` | `isolation.timeout_secs` |
| `PHALUS_ISOLATION__NETWORK_MODE` | `isolation.network_mode` |
| `PHALUS_ISOLATION__PIDS_LIMIT` | `isolation.pids_limit` |
| `PHALUS_LIMITS__MAX_PACKAGES_PER_JOB` | `limits.max_packages_per_job` |
| `PHALUS_LIMITS__CONCURRENCY` | `limits.concurrency` |
| `PHALUS_VALIDATION__SIMILARITY_THRESHOLD` | `validation.similarity_threshold` |
| `PHALUS_VALIDATION__RUN_TESTS` | `validation.run_tests` |
| `PHALUS_VALIDATION__SYNTAX_CHECK` | `validation.syntax_check` |
| `PHALUS_OUTPUT__DEFAULT_LICENSE` | `output.default_license` |
| `PHALUS_OUTPUT__OUTPUT_DIR` | `output.output_dir` |
| `PHALUS_WEB__ENABLED` | `web.enabled` |
| `PHALUS_WEB__HOST` | `web.host` |
| `PHALUS_WEB__PORT` | `web.port` |
| `PHALUS_DOC_FETCHER__GITHUB_TOKEN` | `doc_fetcher.github_token` |
| `PHALUS_DOC_FETCHER__MAX_README_SIZE_KB` | `doc_fetcher.max_readme_size_kb` |
| `PHALUS_DOC_FETCHER__MAX_CODE_EXAMPLE_LINES` | `doc_fetcher.max_code_example_lines` |

Environment variables are applied after the config file is loaded, so they always take precedence. Boolean values accept `true`/`false`. Integer and float values are parsed directly.

### Example

```bash
export PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..."
export PHALUS_LLM__AGENT_B_API_KEY="sk-ant-..."
export PHALUS_ISOLATION__MODE="process"
export PHALUS_VALIDATION__SIMILARITY_THRESHOLD="0.60"

phalus run package.json
```

---

## Precedence

From lowest to highest priority:

1. Built-in defaults (always applied)
2. `~/.phalus/config.toml`
3. `PHALUS_*` environment variables
4. CLI flags (`--license`, `--isolation`, `--similarity-threshold`, etc.)
