# API Reference

The PHALUS web server exposes a REST API used by the web UI. The same endpoints can be called directly with any HTTP client.

Base URL: `http://127.0.0.1:3000` (default; configurable via `--host` and `--port`).

All request and response bodies are JSON unless otherwise noted. All timestamps are RFC 3339 UTC strings.

---

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/health` | Server health check |
| `POST` | `/api/manifest/parse` | Parse a manifest and return the package list |
| `POST` | `/api/jobs` | Start a clean room job |
| `GET` | `/api/jobs/{id}/stream` | Stream real-time job progress via SSE |
| `GET` | `/api/jobs/{id}/download` | Download completed job output as a ZIP |
| `GET` | `/api/packages/{name}/csp` | Get the CSP specification for a package |
| `GET` | `/api/packages/{name}/audit` | Get audit log entries for a package |
| `GET` | `/api/packages/{name}/code` | Get generated source files for a package |

---

## `GET /api/health`

Returns the server status. Use this to confirm the server is running before submitting jobs.

### Response `200 OK`

```json
{
  "status": "ok"
}
```

---

## `POST /api/manifest/parse`

Parse the raw text of a manifest file and return a structured list of packages. PHALUS tries each parser (npm, PyPI, Cargo, Go) in order and returns the first non-empty result.

### Request

| Header | Value |
|--------|-------|
| `Content-Type` | `text/plain` (recommended) or omit |

Body: raw manifest file content as a string.

**Example — npm `package.json`:**

```
POST /api/manifest/parse
Content-Type: text/plain

{
  "name": "my-project",
  "dependencies": {
    "lodash": "^4.17.21",
    "chalk": "^5.3.0"
  }
}
```

### Response `200 OK`

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

| Field | Type | Description |
|-------|------|-------------|
| `manifest_type` | string | Detected manifest format |
| `packages` | array | Parsed dependency list |
| `packages[].name` | string | Package name |
| `packages[].version_constraint` | string | Version specifier as written in the manifest |
| `packages[].ecosystem` | string | `npm`, `pypi`, `crates`, or `go` |

### Response `400 Bad Request`

```json
{
  "error": "could not parse manifest"
}
```

Returned when no parser recognises the input or all parsers return an empty package list.

---

## `POST /api/jobs`

Start a clean room job. The job runs asynchronously in the background. The response returns a `job_id` immediately.

### Request

```json
{
  "manifest_content": "<raw manifest text>",
  "license": "mit",
  "isolation": "context"
}
```

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `manifest_content` | string | Yes | — | Raw manifest text (same format as `/api/manifest/parse`) |
| `license` | string | No | `mit` | SPDX license identifier for generated code. Options: `mit`, `apache-2.0`, `bsd-2`, `bsd-3`, `isc`, `unlicense`, `cc0` |
| `isolation` | string | No | `context` | Isolation mode: `context`, `process`, `container` |

### Response `200 OK`

```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

The `job_id` is a UUID v4 string. Use it with the stream, download, and package endpoints.

### Response `400 Bad Request`

```json
{
  "error": "could not parse manifest"
}
```

---

## `GET /api/jobs/{id}/stream`

Subscribe to real-time progress events for a running or recently completed job using Server-Sent Events (SSE). The connection remains open until the job completes (`JobDone` event) or the client disconnects.

### Path parameters

| Parameter | Description |
|-----------|-------------|
| `id` | Job ID returned by `POST /api/jobs` |

### Response headers

```
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive
```

### Event format

Each event is delivered as an SSE `data` line containing a JSON object. The object has a single key whose name is the event type, and whose value is the event payload.

#### `PackageStarted`

Emitted when a package begins processing.

```
data: {"PackageStarted":{"name":"lodash"}}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Package name |

#### `PhaseDone`

Emitted each time a pipeline phase completes for a package.

```
data: {"PhaseDone":{"name":"lodash","phase":"resolve"}}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Package name |
| `phase` | string | Completed phase: `resolve`, `docs`, `analyze`, `firewall`, `build`, `validate` |

#### `PackageDone`

Emitted when all pipeline stages for a package have completed.

```
data: {"PackageDone":{"name":"lodash","success":true}}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Package name |
| `success` | boolean | `true` if the package passed all validation checks |

#### `JobDone`

Emitted once when all packages in the job have been processed. The stream closes after this event.

```
data: {"JobDone":{"total":3,"failed":0}}
```

| Field | Type | Description |
|-------|------|-------------|
| `total` | integer | Total packages processed |
| `failed` | integer | Number of packages with a FAIL verdict |

### Keep-alive

The server sends SSE keep-alive comments at regular intervals to prevent proxy and browser timeouts. These appear as lines beginning with `:` and can be ignored.

### Response `404 Not Found`

```json
{
  "error": "job not found"
}
```

### Example (curl)

```bash
curl -N "http://127.0.0.1:3000/api/jobs/550e8400-e29b-41d4-a716-446655440000/stream"
```

### Example (JavaScript)

```javascript
const source = new EventSource(
  `/api/jobs/${jobId}/stream`
);

source.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if ('JobDone' in data) {
    source.close();
  }
  console.log(data);
};
```

---

## `GET /api/jobs/{id}/download`

Download all output for a completed job as a ZIP archive.

### Path parameters

| Parameter | Description |
|-----------|-------------|
| `id` | Job ID |

### Response `200 OK`

```
Content-Type: application/zip
Content-Disposition: attachment; filename="phalus-output.zip"
```

Binary ZIP data. The archive mirrors the full output directory structure, including CSP documents, generated source files, and the audit log.

### Response `404 Not Found`

Job not found, or the output directory does not exist.

### Response `409 Conflict`

```
job still running
```

The job has not yet completed. Subscribe to the stream endpoint and wait for `JobDone` before requesting the download.

---

## `GET /api/packages/{name}/csp`

Return the CSP specification manifest for a package that has completed processing. The manifest includes all ten CSP documents with their content and SHA-256 hashes.

### Path parameters

| Parameter | Description |
|-----------|-------------|
| `name` | Package name (as it appears in the output directory) |

### Response `200 OK`

```json
{
  "package_name": "lodash",
  "package_version": "4.17.21",
  "generated_at": "2026-03-26T10:00:03Z",
  "documents": [
    {
      "filename": "01-overview.md",
      "content": "# lodash\n\nA modern JavaScript utility library...",
      "content_hash": "a3f2c1d4..."
    },
    {
      "filename": "02-api-surface.json",
      "content": "{\"functions\": [\"chunk\", \"compact\", ...]}",
      "content_hash": "b5c6d7e8..."
    }
  ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `package_name` | string | Package name |
| `package_version` | string | Resolved version |
| `generated_at` | string | RFC 3339 timestamp when Agent A produced the spec |
| `documents` | array | The ten CSP documents |
| `documents[].filename` | string | Document filename (e.g. `01-overview.md`) |
| `documents[].content` | string | Document content |
| `documents[].content_hash` | string | SHA-256 hex digest of the content |

### Response `404 Not Found`

```json
{
  "error": "CSP manifest not found"
}
```

### Response `500 Internal Server Error`

```json
{
  "error": "invalid CSP manifest JSON"
}
```

---

## `GET /api/packages/{name}/audit`

Return audit log entries relevant to a specific package, filtered from the global audit log.

### Path parameters

| Parameter | Description |
|-----------|-------------|
| `name` | Package name |

### Response `200 OK`

An array of audit log entries where the event's `package` field contains the given name.

```json
[
  {
    "timestamp": "2026-03-26T10:00:01Z",
    "seq": 1,
    "event": {
      "type": "docs_fetched",
      "package": "lodash@4.17.21",
      "urls_accessed": [
        "https://api.github.com/repos/lodash/lodash/readme"
      ],
      "content_hashes": {
        "readme": "b4c1a2d3..."
      }
    }
  },
  {
    "timestamp": "2026-03-26T10:00:03Z",
    "seq": 3,
    "event": {
      "type": "firewall_crossing",
      "package": "lodash@4.17.21",
      "documents_transferred": ["01-overview.md", "02-api-surface.json"],
      "sha256_checksums": {"01-overview.md": "c1d2e3f4..."},
      "isolation_mode": "context",
      "source_code_accessed": false
    }
  }
]
```

See [Audit Trail](audit-trail.md) for a complete description of all event types and fields.

### Response `404 Not Found`

```json
{
  "error": "audit log not found"
}
```

---

## `GET /api/packages/{name}/code`

Return the generated source files for a package as a JSON object. The `.cleanroom/` directory is excluded.

### Path parameters

| Parameter | Description |
|-----------|-------------|
| `name` | Package name |

### Response `200 OK`

A JSON object where keys are relative file paths and values are file contents as strings.

```json
{
  "package.json": "{\n  \"name\": \"lodash\",\n  \"version\": \"4.17.21\",\n  ...\n}",
  "LICENSE": "MIT License\n\nCopyright (c) ...",
  "README.md": "# lodash\n\n...",
  "src/index.js": "// MIT License\n// ...\n\nmodule.exports = { ... };",
  "test/index.test.js": "const assert = require('assert');\n..."
}
```

Binary files that cannot be decoded as UTF-8 are omitted.

### Response `404 Not Found`

```json
{
  "error": "package output not found"
}
```

### Response `500 Internal Server Error`

```json
{
  "error": "failed to read files: <OS error>"
}
```

---

## Error Responses

All error responses use the same envelope:

```json
{
  "error": "<human-readable message>"
}
```

HTTP status codes follow standard conventions:

| Status | Meaning |
|--------|---------|
| `200` | Success |
| `400` | Bad request (invalid input) |
| `404` | Resource not found |
| `409` | Conflict (e.g. job still running) |
| `500` | Internal server error |
