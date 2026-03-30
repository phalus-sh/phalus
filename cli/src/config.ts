import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

export interface PhalusConfig {
  /** Policy name or ID to enforce (e.g. "permissive-only"). */
  policy?: string;
  /** Whether to exit 1 on policy violations. Default: true. */
  failOnViolation?: boolean;
  /** Directories to scan. Default: ["."] */
  paths?: string[];
  /** Ecosystem to restrict scanning to, or "auto". Default: "auto" (all). */
  ecosystem?: string;
}

/**
 * Parse the subset of YAML used in .phalus.yml.
 * Handles the exact schema defined in the spec — no general-purpose YAML needed.
 */
export function parseConfig(content: string): PhalusConfig {
  const config: PhalusConfig = {};
  const lines = content.split('\n');
  let inPaths = false;

  for (const rawLine of lines) {
    // Strip inline comments
    const stripped = rawLine.replace(/#.*$/, '');
    const trimmed = stripped.trim();
    if (!trimmed) {
      // A blank line ends a sequence
      if (inPaths) inPaths = false;
      continue;
    }

    // List item: must start with "- " (with at least one leading space if inside mapping)
    if (inPaths) {
      const listMatch = stripped.match(/^\s+-\s+(.*)/);
      if (listMatch) {
        (config.paths ??= []).push(listMatch[1]!.trim());
        continue;
      }
      inPaths = false;
    }

    const kv = trimmed.match(/^([a-zA-Z][a-zA-Z0-9]*):\s*(.*)$/);
    if (!kv) continue;

    const key = kv[1]!;
    const val = kv[2]!.trim();

    switch (key) {
      case 'policy':
        if (val) config.policy = val;
        break;
      case 'failOnViolation':
        config.failOnViolation = val === 'true';
        break;
      case 'ecosystem':
        if (val && val !== 'auto') config.ecosystem = val;
        break;
      case 'paths':
        config.paths = [];
        inPaths = true;
        break;
    }
  }

  return config;
}

/**
 * Load .phalus.yml from the given directory.
 * Returns an empty config object if the file does not exist or cannot be parsed.
 */
export function loadConfig(dir: string): PhalusConfig {
  const configPath = join(dir, '.phalus.yml');
  if (!existsSync(configPath)) return {};
  try {
    const content = readFileSync(configPath, 'utf-8');
    return parseConfig(content);
  } catch {
    return {};
  }
}
