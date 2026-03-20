#!/usr/bin/env node
/**
 * PostToolUse Hook: Trigger rhizome export and hyphae re-indexing after changes
 *
 * Cross-platform (Windows, macOS, Linux)
 *
 * Tracks file edits (Write/Edit/MultiEdit tools) in two pending lists:
 * 1. All files for Rhizome export (triggers after 5+ edits on build success)
 * 2. Document files for Hyphae ingestion (triggers after 3+ edits)
 *
 * Detects successful builds and triggers both asynchronously without blocking
 * Claude Code.
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

// ─────────────────────────────────────────────────────────────────────────
// Document file extensions for Hyphae ingestion
// ─────────────────────────────────────────────────────────────────────────
const DOCUMENT_EXTENSIONS = new Set([
  '.md', '.txt', '.rst', '.adoc',        // Documentation
  '.json', '.yaml', '.yml', '.toml',    // Config/Data
  '.html', '.css',                       // Web
  '.env', '.cfg', '.ini',                // Environment/Config
  '.sh', '.sql'                          // Scripts/Queries
]);

const EXPORT_THRESHOLD = 5;
const INGEST_THRESHOLD = 3;

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
  // Track file edits (for both Rhizome and Hyphae)
  // ─────────────────────────────────────────────────────────────────────────
  if (TRACKED_WRITE_TOOLS.includes(toolName) && filePath) {
    trackFileEdit(filePath);

    // Also track document files separately for Hyphae
    if (isDocumentFile(filePath)) {
      trackDocumentEdit(filePath);
    }
  }

  // ─────────────────────────────────────────────────────────────────────────
  // Detect build success and trigger exports
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

  // ─────────────────────────────────────────────────────────────────────────
  // Check if Hyphae ingest threshold is met (lower threshold, no build needed)
  // ─────────────────────────────────────────────────────────────────────────
  const pendingDocuments = getPendingDocuments();
  if (pendingDocuments.length >= INGEST_THRESHOLD && commandExists('hyphae')) {
    triggerHyphaePending();
    clearPendingDocuments();
  }
}

// ─────────────────────────────────────────────────────────────────────────
// Rhizome Export Tracking
// ─────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────
// Hyphae Document Ingestion Tracking
// ─────────────────────────────────────────────────────────────────────────

function isDocumentFile(filePath) {
  if (!filePath) return false;
  const ext = path.extname(filePath).toLowerCase();
  return DOCUMENT_EXTENSIONS.has(ext);
}

function getPendingDocumentsPath() {
  const cwdHash = crypto.createHash('sha256').update(process.cwd()).digest('hex').slice(0, 12);
  return path.join('/tmp', `hyphae-pending-ingest-${cwdHash}.txt`);
}

function getPendingDocuments() {
  const filePath = getPendingDocumentsPath();
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

function trackDocumentEdit(filePath) {
  const pendingPath = getPendingDocumentsPath();
  try {
    fs.appendFileSync(pendingPath, `${filePath}\n`, 'utf8');
  } catch {
    // Non-critical — if we can't track, we just skip ingest
  }
}

function clearPendingDocuments() {
  const filePath = getPendingDocumentsPath();
  try {
    if (fs.existsSync(filePath)) {
      fs.unlinkSync(filePath);
    }
  } catch {
    // Non-critical
  }
}

function triggerHyphaePending() {
  try {
    const documents = getPendingDocuments();
    if (documents.length === 0) {
      return;
    }

    // Spawn hyphae ingest-file for each document asynchronously — fire and forget
    for (const filePath of documents) {
      try {
        const child = spawn('hyphae', ['ingest-file', filePath], {
          detached: true,
          stdio: ['pipe', 'pipe', 'pipe']
        });

        // Detach from parent process so Claude Code doesn't wait
        if (child.unref) {
          child.unref();
        }
      } catch (err) {
        logHookError('capture-code-changes-hyphae', err);
      }
    }

    log(`[capture-code-changes] Triggered hyphae ingest-file for ${documents.length} document(s)`);
  } catch (err) {
    logHookError('capture-code-changes-hyphae', err);
  }
}
