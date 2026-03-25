# phalus

**Private Headless Automated License Uncoupling System** — phalus generates clean-room reimplementations of open source packages using LLM agents, isolated behind a strict information firewall.

## Build

```sh
cargo build --release -j2
```

## Quick Start

Analyze all packages in a manifest and generate a plan:

```sh
phalus plan package.json
```

Run a single package through the full pipeline:

```sh
phalus run-one npm/left-pad@1.1.3 --license mit
```

Run all packages in a manifest:

```sh
phalus run package.json --license apache-2.0 --output ./phalus-output
```

## Configuration

Global configuration lives at `~/.phalus/config.toml`:

```toml
[llm]
provider = "anthropic"
model = "claude-opus-4-5"
api_key_env = "ANTHROPIC_API_KEY"

[defaults]
license = "mit"
output_dir = "./phalus-output"
similarity_threshold = 0.70
concurrency = 3

[isolation]
mode = "context"
```

## Ethical Notice

This tool raises serious ethical and legal questions about open source sustainability. It exists for research, education, and transparent discourse — not to encourage license evasion. You are responsible for understanding the legal implications in your jurisdiction. The legality of AI-assisted clean room reimplementation is unsettled law.

## License

0BSD
