#!/usr/bin/env node
/**
 * PostToolUse Hook: Trigger rhizome export after significant code changes
 *
 * Cross-platform (Windows, macOS, Linux)
 *
 * Tracks file edits (Write/Edit/MultiEdit tools) in a pending list and detects
 * successful builds (cargo build, npm run build, tsc, etc.). When a build succeeds
 * and 5+ unique files have been modified, triggers `rhizome export` asynchronously
 * without blocking Claude Code.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawn } = require('child_process');
const { log, commandExists, getProjectName, logHookError } = require('../lib/utils');

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

const TRACKED_WRITE_TOOLS = ['Write', 'Edit', 'MultiEdit'];
const BUILD_COMMANDS = [
  /\bcargo\s+(build|check)\b/,
  /\bnpm\s+run\s+build\b/,
  /\byarn\s+build\b/,
  /\bpnpm\s+build\b/,
  /\bbun\s+build\b/,
  /\btsc\b/,
  /\bnext\s+build\b/,
  /\bmake\b/,
  /\bgo\s+build\b/,
  /\bgradlew\s+build\b/,
  /\bmvn\s+clean\s+package\b/
];

const EXPORT_THRESHOLD = 5;

function processToolUse() {
  let input;
  try {
    input = JSON.parse(data);
  } catch {
    return;
  }

  const toolName = input.tool_name || '';
  const toolInput = input.tool_input || {};
  const toolOutput = input.tool_output || {};
  const filePath = toolInput.file_path;
  const command = toolInput.command || '';
  const exitCode = toolOutput.exit_code;

  // ─────────────────────────────────────────────────────────────────────────
  // Track file edits
  // ─────────────────────────────────────────────────────────────────────────
  if (TRACKED_WRITE_TOOLS.includes(toolName) && filePath) {
    trackFileEdit(filePath);
  }

  // ─────────────────────────────────────────────────────────────────────────
  // Detect build success and trigger export
  // ─────────────────────────────────────────────────────────────────────────
  if (toolName === 'Bash' && command) {
    const isBuildCommand = BUILD_COMMANDS.some(p => p.test(command));
    const isBuildSuccess = exitCode === 0 || exitCode === undefined;

    if (isBuildCommand && isBuildSuccess) {
      const pendingFiles = getPendingFiles();
      if (pendingFiles.length >= EXPORT_THRESHOLD && commandExists('rhizome')) {
        triggerRhizomeExport();
        clearPendingFiles();
      }
    }
  }
}

function getPendingFilesPath() {
  const cwdHash = crypto.createHash('sha256').update(process.cwd()).digest('hex').slice(0, 12);
  return path.join('/tmp', `rhizome-pending-exports-${cwdHash}.txt`);
}

function getPendingFiles() {
  const filePath = getPendingFilesPath();
  try {
    if (fs.existsSync(filePath)) {
      const content = fs.readFileSync(filePath, 'utf8');
      const files = content.split('\n').filter(Boolean);
      // Return unique files
      return [...new Set(files)];
    }
  } catch {
    // Non-critical
  }
  return [];
}

function trackFileEdit(filePath) {
  const pendingPath = getPendingFilesPath();
  try {
    fs.appendFileSync(pendingPath, `${filePath}\n`, 'utf8');
  } catch {
    // Non-critical — if we can't track, we just skip export
  }
}

function clearPendingFiles() {
  const filePath = getPendingFilesPath();
  try {
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
    }
  } catch {
    // Non-critical
  }
}

function triggerRhizomeExport() {
  try {
    // Spawn rhizome export asynchronously — don't wait for completion
    const child = spawn('rhizome', ['export'], {
      detached: true,
      stdio: ['pipe', 'pipe', 'pipe']
    });

    // Detach from parent process so Claude Code doesn't wait
    if (child.unref) {
      child.unref();
    }

    log('[capture-code-changes] Triggered rhizome export asynchronously');
  } catch (err) {
    logHookError('capture-code-changes', err);
  }
}
