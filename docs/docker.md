# Docker

PHALUS provides an official Docker image for users who prefer not to install Rust toolchains, or who want to run PHALUS in a consistent environment.

---

## Image

```
ghcr.io/phalus-project/phalus:latest
```

Tags:

| Tag | Description |
|-----|-------------|
| `latest` | Latest stable release |
| `x.y.z` | Specific version |
| `main` | Built from the main branch (may be unstable) |

---

## Building the Image Locally

To build from source:

```bash
git clone https://github.com/phalus-project/phalus.git
cd phalus
docker build -t phalus:local .
```

The image is built on `debian:bookworm-slim`. The PHALUS binary is compiled in a separate build stage and copied into the final image. The container runs as a non-root user (`phalus`, UID 1000) by default.

---

## Running the Container

### Basic run with environment variables

```bash
docker run --rm \
  -e PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..." \
  -e PHALUS_LLM__AGENT_B_API_KEY="sk-ant-..." \
  -v "$PWD":/work \
  -w /work \
  ghcr.io/phalus-project/phalus:latest \
  run package.json --license mit
```

### Shell alias for convenience

Add to your shell profile:

```bash
alias phalus='docker run --rm \
  -e PHALUS_LLM__AGENT_A_API_KEY \
  -e PHALUS_LLM__AGENT_B_API_KEY \
  -v "$PWD":/work \
  -v "$HOME/.phalus":/home/phalus/.phalus \
  -w /work \
  ghcr.io/phalus-project/phalus:latest'
```

This mounts the current directory as `/work` and the host `~/.phalus/` for persistent configuration and CSP cache. The `PHALUS_LLM__*` environment variables are forwarded from the host shell.

---

## Environment Variables

All `PHALUS_*` environment variables are supported. The most commonly needed ones when running in Docker:

| Variable | Description |
|----------|-------------|
| `PHALUS_LLM__AGENT_A_API_KEY` | API key for Agent A (required) |
| `PHALUS_LLM__AGENT_B_API_KEY` | API key for Agent B (required) |
| `PHALUS_LLM__AGENT_A_MODEL` | Override Agent A model |
| `PHALUS_LLM__AGENT_B_MODEL` | Override Agent B model |
| `PHALUS_ISOLATION__MODE` | Isolation mode: `context`, `process`, `container` |
| `PHALUS_DOC_FETCHER__GITHUB_TOKEN` | GitHub token for higher rate limits |

See [Configuration](configuration.md#environment-variable-overrides) for the full list.

---

## Port Mapping

To use the web UI from a Docker container:

```bash
docker run --rm \
  -e PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..." \
  -e PHALUS_LLM__AGENT_B_API_KEY="sk-ant-..." \
  -p 127.0.0.1:3000:3000 \
  -v "$PWD":/work \
  -w /work \
  ghcr.io/phalus-project/phalus:latest \
  serve --host 0.0.0.0 --port 3000
```

The `-p 127.0.0.1:3000:3000` flag binds the container port to the local loopback only. Open `http://127.0.0.1:3000` in a browser.

Note: inside the container, `--host 0.0.0.0` is required so the server listens on all container interfaces. The external binding to `127.0.0.1` is controlled by Docker.

---

## Volume Mounts for Persistent Output

Without a volume mount, output is written inside the container and lost when it exits. Mount a host directory as the output path:

```bash
docker run --rm \
  -e PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..." \
  -e PHALUS_LLM__AGENT_B_API_KEY="sk-ant-..." \
  -v "$PWD":/work \
  -v "$PWD/phalus-output":/output \
  -w /work \
  ghcr.io/phalus-project/phalus:latest \
  run package.json --output /output
```

The generated packages, CSP specifications, and audit log will be available at `./phalus-output/` on the host after the container exits.

To persist the CSP cache across runs (avoids re-running Agent A for unchanged packages):

```bash
docker run --rm \
  -e PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..." \
  -e PHALUS_LLM__AGENT_B_API_KEY="sk-ant-..." \
  -v "$HOME/.phalus":/home/phalus/.phalus \
  -v "$PWD":/work \
  -w /work \
  ghcr.io/phalus-project/phalus:latest \
  run package.json
```

---

## Docker-Based Test Execution

When `validation.run_tests = true`, PHALUS attempts to run generated tests. Inside Docker, the relevant language runtime must be available. The PHALUS image includes the Node.js and Python runtimes for this purpose. Rust and Go test execution requires the appropriate toolchain to be present.

To run tests for Rust output:

```bash
docker run --rm \
  -e PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..." \
  -e PHALUS_LLM__AGENT_B_API_KEY="sk-ant-..." \
  -v "$PWD":/work \
  -w /work \
  ghcr.io/phalus-project/phalus:latest-rust \
  run-one crates/serde@1.0.193
```

The `-rust` image variant includes the Rust toolchain. Check the releases page for available image variants.

---

## Container Isolation Mode

When `--isolation container` is specified, PHALUS spawns separate Docker containers for Agent A and Agent B. In this mode, the host Docker socket must be mounted:

```bash
docker run --rm \
  -e PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..." \
  -e PHALUS_LLM__AGENT_B_API_KEY="sk-ant-..." \
  -v /var/run/docker.sock:/var/run/docker.sock \
  -v "$PWD":/work \
  -w /work \
  ghcr.io/phalus-project/phalus:latest \
  run package.json --isolation container
```

In container isolation mode, each agent runs in its own container with no shared network. Only the CSP documents are written to a shared volume that Agent B's container can read. This provides the strongest isolation guarantee and the most defensible audit trail.

Mounting the Docker socket grants significant privilege. Only do this in environments where you trust the PHALUS image.

---

## Non-Root User

The container runs as user `phalus` (UID 1000, GID 1000) by default. If the mounted host directories are owned by a different UID, files may not be writable. Fix with:

```bash
# Option 1: run as the current host user
docker run --rm \
  --user "$(id -u):$(id -g)" \
  -e PHALUS_LLM__AGENT_A_API_KEY="sk-ant-..." \
  -v "$PWD":/work \
  -w /work \
  ghcr.io/phalus-project/phalus:latest \
  run package.json

# Option 2: make the output directory world-writable before running
mkdir -p ./phalus-output && chmod 777 ./phalus-output
```

---

## Docker Compose

Example `docker-compose.yml` for running the web UI as a persistent service:

```yaml
services:
  phalus:
    image: ghcr.io/phalus-project/phalus:latest
    command: serve --host 0.0.0.0 --port 3000
    environment:
      PHALUS_LLM__AGENT_A_API_KEY: "${PHALUS_LLM__AGENT_A_API_KEY}"
      PHALUS_LLM__AGENT_B_API_KEY: "${PHALUS_LLM__AGENT_B_API_KEY}"
    ports:
      - "127.0.0.1:3000:3000"
    volumes:
      - ./phalus-output:/output
      - phalus-cache:/home/phalus/.phalus

volumes:
  phalus-cache:
```

Run with:

```bash
PHALUS_LLM__AGENT_A_API_KEY=sk-ant-... \
PHALUS_LLM__AGENT_B_API_KEY=sk-ant-... \
docker compose up
```
