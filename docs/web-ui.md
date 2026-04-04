# Web UI

PHALUS includes an optional browser-based interface for users who prefer not to work directly with the CLI. It is a single-page application served by the same process that runs the pipeline.

The web UI is a convenience wrapper. Every operation it performs is backed by the same pipeline and produces the same audit trail as the CLI.

---

## Starting the Server

```bash
phalus serve
```

By default the server binds to `127.0.0.1:3000`. It is accessible only from the local machine. There is no authentication, no sessions, and no cookies. If you can reach the port, you have full access.

Options:

```bash
phalus serve --host 127.0.0.1 --port 3000
```

Open `http://127.0.0.1:3000` in a browser.

To start the server automatically when PHALUS runs, set `web.enabled = true` in `~/.phalus/config.toml`. See [Configuration](configuration.md#web) for details.

---

## Views

### Home

A drop zone for pasting or uploading a manifest file, and a text input for the `run-one` shortcut. Entering `npm/lodash@4.17.21` and clicking Run launches a single-package job without a manifest.

### Plan

After uploading a manifest, the Plan view displays a table of parsed packages: name, version, original ecosystem, and original license. Each row has a checkbox to include or exclude the package from the job. A license selector and Start button appear at the bottom.

### Progress

Once a job starts, the Progress view shows a live status indicator for each package. Phase transitions are streamed as Server-Sent Events:

```
fetching docs → analyzing → firewall → implementing → validating → done
```

Each phase indicator updates in real time as the backend emits `PhaseDone` events.

### Results

After the job completes, per-package cards appear with:

- **Verdict** — PASS or FAIL, with the similarity score
- **Download code** — Download that package's generated source
- **View CSP** — Inspect the ten specification documents Agent A produced
- **View audit** — Filter the audit log to events for this package
- **Bulk download** — Download all packages as a single ZIP archive (`GET /api/jobs/{id}/download`)

---

## API Endpoints

The web UI is backed by a REST API that can also be used directly. All endpoints return JSON unless otherwise noted.

### `GET /api/health`

Returns the server status.

**Response:**

```json
{"status": "ok"}
```

---

### `POST /api/manifest/parse`

Parse a manifest file body and return the list of detected packages. The body is the raw manifest text. PHALUS tries each parser (npm, PyPI, Cargo, Go) in order and returns the first non-empty result.

**Request:**

```
Content-Type: text/plain

{
  "name": "my-app",
  "dependencies": {
    "lodash": "^4.17.21",
    "chalk": "^5.3.0"
  }
}
```

**Response (200):**

```json
{
  "manifest_type": "package.json",
  "packages": [
    {
      "name": "lodash",
      "version_constraint": "^4.17.21",
      "ecosystem": "npm"
    },
    {
      "name": "chalk",
      "version_constraint": "^5.3.0",
      "ecosystem": "npm"
    }
  ]
}
```

**Response (400):**

```json
{"error": "could not parse manifest"}
```

---

### `POST /api/jobs`

Start a clean room job. Returns a `job_id` immediately; the job runs in the background.

**Request body:**

```json
{
  "manifest_content": "<raw manifest text>",
  "license": "mit",
  "isolation": "context"
}
```

| Field | Required | Default | Description |
|-------|----------|---------|-------------|
| `manifest_content` | Yes | — | Raw manifest text |
| `license` | No | `mit` | Output license identifier |
| `isolation` | No | `context` | Isolation mode: `context`, `process`, `container` |

**Response (200):**

```json
{"job_id": "550e8400-e29b-41d4-a716-446655440000"}
```

**Response (400):**

```json
{"error": "could not parse manifest"}
```

---

### `GET /api/jobs/{id}/stream`

Subscribe to real-time progress events for a job using Server-Sent Events (SSE). The connection stays open until a `JobDone` event is received.

**Response headers:**

```
Content-Type: text/event-stream
Cache-Control: no-cache
```

**Event format:**

Each event is a JSON object in the SSE `data` field. Event types:

| Type | Fields |
|------|--------|
| `PackageStarted` | `name` |
| `PhaseDone` | `name`, `phase` |
| `PackageDone` | `name`, `success` |
| `JobDone` | `total`, `failed` |

Phase values for `PhaseDone`: `resolve`, `docs`, `analyze`, `firewall`, `build`, `validate`.

**Example stream:**

```
data: {"PackageStarted":{"name":"lodash"}}

data: {"PhaseDone":{"name":"lodash","phase":"resolve"}}

data: {"PhaseDone":{"name":"lodash","phase":"docs"}}

data: {"PhaseDone":{"name":"lodash","phase":"analyze"}}

data: {"PhaseDone":{"name":"lodash","phase":"firewall"}}

data: {"PhaseDone":{"name":"lodash","phase":"build"}}

data: {"PhaseDone":{"name":"lodash","phase":"validate"}}

data: {"PackageDone":{"name":"lodash","success":true}}

data: {"JobDone":{"total":1,"failed":0}}
```

The stream closes automatically after `JobDone`. The server sends periodic keep-alive comments to prevent proxy timeouts.

**Response (404):**

```json
{"error": "job not found"}
```

---

### `GET /api/jobs/{id}/download`

Download all output for a completed job as a ZIP archive.

**Response (200):**

```
Content-Type: application/zip
Content-Disposition: attachment; filename="phalus-output.zip"

<binary ZIP data>
```

**Response (404):** Job not found.

**Response (409):** Job is still running (`job still running`).

---

### `GET /api/packages/{name}/csp`

Return the CSP manifest JSON for a completed package. The `name` is the package name as it appears in the output directory.

**Response (200):**

```json
{
  "package_name": "lodash",
  "package_version": "4.17.21",
  "generated_at": "2026-03-26T10:00:03Z",
  "documents": [
    {
      "filename": "01-overview.json",
      "content": "# lodash\n...",
      "content_hash": "a3f2..."
    },
    ...
  ]
}
```

**Response (404):** CSP manifest not found for the package.

---

### `GET /api/packages/{name}/audit`

Return audit log entries relevant to a specific package, filtered from the global `audit.jsonl` file.

**Response (200):**

```json
[
  {
    "timestamp": "2026-03-26T10:00:01Z",
    "seq": 1,
    "event": {
      "type": "docs_fetched",
      "package": "lodash@4.17.21",
      "urls_accessed": ["https://api.github.com/repos/lodash/lodash/readme"],
      "content_hashes": {"readme": "b4c1..."}
    }
  },
  ...
]
```

**Response (404):** Audit log not found.

---

### `GET /api/packages/{name}/code`

Return the generated source files for a package as a JSON object mapping relative paths to file contents. The `.cleanroom/` directory is excluded.

**Response (200):**

```json
{
  "package.json": "{\"name\": \"lodash\", ...}",
  "LICENSE": "MIT License...",
  "src/index.js": "// MIT License\n...",
  "test/index.test.js": "..."
}
```

**Response (404):** Package output directory not found.

---

## Security Notes

The web server binds to `127.0.0.1` by default. If you change the host to `0.0.0.0` or expose the port externally, anyone who can reach it has full control of the pipeline, including the ability to trigger LLM API calls that consume your API quota. There is no authentication layer. Do not expose the server to untrusted networks.

See [Security](security.md) for additional details.
