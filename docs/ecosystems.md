# Supported Ecosystems

PHALUS currently supports four package ecosystems end-to-end: npm, PyPI, Cargo, and Go. Additional ecosystems are planned.

---

## Supported Today

| Ecosystem | Manifest File | Registry | Registry API |
|-----------|--------------|----------|--------------|
| npm (Node.js) | `package.json` | registry.npmjs.org | `https://registry.npmjs.org/{name}/{version}` |
| PyPI (Python) | `requirements.txt`, `pyproject.toml` | pypi.org | `https://pypi.org/pypi/{name}/{version}/json` |
| Cargo (Rust) | `Cargo.toml` | crates.io | `https://crates.io/api/v1/crates/{name}/{version}` |
| Go | `go.mod` | proxy.golang.org | `https://proxy.golang.org/{module}/@v/{version}.info` |

---

## Ecosystem Details

### npm

**Manifest parsing:** Reads `dependencies` and `devDependencies` from `package.json`. Version constraints follow npm semver notation (`^`, `~`, `>=`, exact).

**Registry resolution:** Fetches `https://registry.npmjs.org/{name}/{version}` and extracts:
- `dist.unpackedSize` — package size estimate
- `repository.url` — used to find the GitHub README
- `license` — original license being replaced
- `description`, `homepage`

**Documentation fetching:**
- README from GitHub API (`/repos/{owner}/{repo}/readme`)
- Documentation site at the `homepage` URL if present
- TypeScript type definitions from DefinitelyTyped (`@types/{name}`) when available. For packages that ship their own `.d.ts` files, type definitions are extracted from the npm tarball.

**`run-one` format:** `npm/{name}@{version}` — e.g. `npm/lodash@4.17.21`

---

### PyPI

**Manifest parsing:** Reads `requirements.txt` (one package per line, PEP 508 version specifiers) and `[project.dependencies]` / `[tool.poetry.dependencies]` from `pyproject.toml`.

**Registry resolution:** Fetches `https://pypi.org/pypi/{name}/{version}/json` and extracts:
- `info.description` — long description (may include API docs in Markdown)
- `info.license`
- `info.project_urls` — links to documentation and repository
- `urls[].size` — package size

**Documentation fetching:**
- README from the GitHub repository if `info.project_urls` contains a repository link
- Documentation site if a docs URL is present in `project_urls`

**`run-one` format:** `pypi/{name}@{version}` — e.g. `pypi/requests@2.31.0`

---

### Cargo (Rust)

**Manifest parsing:** Reads `[dependencies]`, `[dev-dependencies]`, and `[build-dependencies]` from `Cargo.toml`. Handles both inline (`serde = "1"`) and table (`serde = { version = "1", features = [...] }`) syntax.

**Registry resolution:** Fetches `https://crates.io/api/v1/crates/{name}/{version}` and extracts:
- `crate.description`
- `crate.homepage`, `crate.documentation`, `crate.repository`
- `version.license`

**Documentation fetching:**
- README from the GitHub repository
- docs.rs documentation page (`https://docs.rs/{name}/{version}/{name}/`) if the crate publishes to docs.rs (nearly all do)

**`run-one` format:** `crates/{name}@{version}` — e.g. `crates/serde@1.0.193`

---

### Go

**Manifest parsing:** Reads `require` directives from `go.mod`. Module paths follow Go module conventions (`github.com/user/repo`, `golang.org/x/...`, etc.).

**Registry resolution:** Fetches `https://proxy.golang.org/{module}/@v/{version}.info` for version confirmation and `https://pkg.go.dev/{module}@{version}` for metadata.

**Documentation fetching:**
- README from the GitHub or other VCS repository
- pkg.go.dev documentation page for the module

**`run-one` format:** `go/{module}@{version}` — e.g. `go/github.com/gin-gonic/gin@v1.9.1`

---

## Metadata Fetched for All Ecosystems

Regardless of ecosystem, the registry resolution stage populates a `PackageMetadata` struct with the following fields, which are used throughout the pipeline:

| Field | Description |
|-------|-------------|
| `name` | Package name |
| `version` | Resolved version string |
| `description` | Short description |
| `license` | Original SPDX license identifier |
| `repository_url` | Link to source repository (used to fetch README) |
| `homepage_url` | Link to documentation site (used to fetch docs) |
| `ecosystem` | Ecosystem tag: `npm`, `pypi`, `crates`, `go` |

---

## Planned Ecosystems

The following ecosystems are on the roadmap but not yet implemented:

| Ecosystem | Manifest | Registry | Status |
|-----------|---------|----------|--------|
| Java / Kotlin | `pom.xml`, `build.gradle` | Maven Central | Planned |
| Ruby | `Gemfile` | rubygems.org | Planned |
| PHP | `composer.json` | packagist.org | Planned |
| .NET | `*.csproj`, `packages.config` | nuget.org | Planned |

---

## Cross-Language Reimplementation

PHALUS can reimplement a package in a different target language regardless of the source ecosystem. The CSP specification produced by Agent A is language-neutral — it describes API surface and behaviour, not syntax.

```bash
# Reimplement a JavaScript npm package in Rust
phalus run-one npm/chalk@5.3.0 --target-lang rust --license mit

# Reimplement a Python package in Go
phalus run-one pypi/requests@2.31.0 --target-lang go
```

Supported target languages: `rust`, `go`, `python`, `typescript`. The default is to implement in the same language as the original package.

Cross-language reimplementation tends to produce lower similarity scores because the language change forces structural divergence independent of model behaviour.
