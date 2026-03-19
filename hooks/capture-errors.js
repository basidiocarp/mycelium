#!/usr/bin/env node
/**
 * PostToolUse Hook: Capture Bash errors in hyphae
 *
 * Cross-platform (Windows, macOS, Linux)
 *
 * Detects command failures from Bash tool output and stores them in hyphae
 * for cross-session recall. Tracks active errors in a temp file and detects
 * resolutions when a previously-failed command succeeds.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawnSync } = require('child_process');
const { log, commandExists, getProjectName } = require('../lib/utils');

const MAX_STDIN = 1024 * 1024;
let data = '';
process.stdin.setEncoding('utf8');

process.stdin.on('data', chunk => {
  if (data.length < MAX_STDIN) {
    const remaining = MAX_STDIN - data.length;
    data += chunk.substring(0, remaining);
  }
});

process.stdin.on('end', () => {
  try {
    processToolUse();
  } catch {
    // Hook must never fail
  }
  process.stdout.write(data);
  process.exit(0);
});

const SIGNIFICANT_COMMANDS = [
  /\bcargo\b/, /\bnpm\b/, /\byarn\b/, /\bpnpm\b/, /\bbun\b/,
  /\bgit\s+push\b/, /\bdocker\b/, /\bpytest\b/, /\bmake\b/,
  /\bgo\s+(build|test|run|vet)\b/, /\brustc\b/, /\bgcc\b/, /\bg\+\+\b/,
  /\bjavac\b/, /\bmvn\b/, /\bgradle\b/, /\bvitest\b/, /\bjest\b/,
  /\bplaywright\b/, /\btsc\b/, /\bpython\b/, /\bruby\b/, /\bswift\b/
];

const ERROR_PATTERNS = [
  /\berror[\s:[\]]/i,
  /\bFAILED\b/,
  /\bpanicked\b/,
  /\bfailed\b/,
  /\bfatal[\s:]/i,
  /\bcommand not found\b/,
  /\bsegmentation fault\b/i,
  /\baborted\b/i
];

function processToolUse() {
  let input;
  try {
    input = JSON.parse(data);
  } catch {
    return;
  }

  const command = input.tool_input?.command || '';
  const output = input.tool_output?.output || '';
  const exitCode = input.tool_output?.exit_code;

  if (!command) return;
  if (!SIGNIFICANT_COMMANDS.some(p => p.test(command))) return;

  const hyphaeAvailable = commandExists('hyphae');
  const cwdHash = crypto.createHash('sha256').update(process.cwd()).digest('hex').slice(0, 12);
  const trackFile = path.join('/tmp', `hyphae-active-errors-${cwdHash}.json`);
  const cmdKey = normalizeCommand(command);
  const hasError = detectError(output, exitCode);

  if (hasError) {
    trackError(cmdKey, command, output, trackFile);
    if (hyphaeAvailable) {
      storeErrorInHyphae(command, output);
    }
  } else {
    resolveError(cmdKey, command, trackFile, hyphaeAvailable);
  }
}

function normalizeCommand(cmd) {
  const parts = cmd.trim().split(/\s+/);
  if (parts.length >= 2) return `${parts[0]} ${parts[1]}`;
  return parts[0] || cmd;
}

function detectError(output, exitCode) {
  if (exitCode !== undefined && exitCode !== null && exitCode !== 0) return true;
  return ERROR_PATTERNS.some(p => p.test(output));
}

function loadTrackFile(trackFile) {
  try {
    if (fs.existsSync(trackFile)) {
      return JSON.parse(fs.readFileSync(trackFile, 'utf8'));
    }
  } catch {
    // Corrupt file — start fresh
  }
  return {};
}

function saveTrackFile(trackFile, entries) {
  try {
    fs.writeFileSync(trackFile, JSON.stringify(entries, null, 2), 'utf8');
  } catch {
    // Non-critical
  }
}

function trackError(cmdKey, command, output, trackFile) {
  const entries = loadTrackFile(trackFile);
  entries[cmdKey] = {
    command: command.slice(0, 500),
    error: output.slice(0, 500),
    timestamp: Date.now()
  };
  saveTrackFile(trackFile, entries);
}

function storeErrorInHyphae(command, output) {
  try {
    const project = getProjectName();
    const content = `Command: ${command.slice(0, 200)}\nError: ${output.slice(0, 500)}`;
    const args = [
      'store', '--topic', 'errors/active',
      '--content', content,
      '--importance', 'medium',
      '--keywords', 'error,active,cli'
    ];
    if (project) args.push('-P', project);
    spawnSync('hyphae', args, {
      encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], timeout: 3000
    });
  } catch {
    // Non-critical
  }
}

function resolveError(cmdKey, command, trackFile, hyphaeAvailable) {
  const entries = loadTrackFile(trackFile);
  if (!entries[cmdKey]) return;

  const previousError = entries[cmdKey];
  delete entries[cmdKey];
  saveTrackFile(trackFile, entries);

  if (hyphaeAvailable) {
    try {
      const project = getProjectName();
      const content = `Fixed: ${command.slice(0, 200)}\nPrevious error: ${previousError.error.slice(0, 300)}`;
      const args = [
        'store', '--topic', 'errors/resolved',
        '--content', content,
        '--importance', 'high',
        '--keywords', 'error,resolved,fix'
      ];
      if (project) args.push('-P', project);
      spawnSync('hyphae', args, {
        encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], timeout: 3000
      });
      log('[capture-errors] Stored error resolution in hyphae');
    } catch {
      // Non-critical
    }
  }
}
