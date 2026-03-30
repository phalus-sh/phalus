#!/usr/bin/env node
// PHALUS CLI — Phase 0 stub

const [, , command, ...args] = process.argv;

function help() {
  console.log(`
phalus <command> [options]

Commands:
  scan <path>    Scan a project directory for license data
  help           Show this help

Examples:
  phalus scan ./my-project
`);
}

switch (command) {
  case 'scan':
    if (!args[0]) {
      console.error('Error: path is required');
      process.exit(1);
    }
    console.log(`[phalus] Scanning ${args[0]} ... (not yet implemented — Phase 1)`);
    break;
  case 'help':
  case '--help':
  case '-h':
  case undefined:
    help();
    break;
  default:
    console.error(`Unknown command: ${command}`);
    help();
    process.exit(1);
}
