#!/usr/bin/env node
/**
 * PostToolUse Hook: Capture self-corrections in hyphae
 *
 * Cross-platform (Windows, macOS, Linux)
 *
 * Detects when an agent corrects its own recent edit (same file edited within
 * 5 minutes, with the second old_string overlapping the first new_string).
 * Stores corrections in hyphae for cross-session pattern recall.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { spawnSync } = require('child_process');
const { log, commandExists, getProjectName } = require('../lib/utils');

const MAX_STDIN = 1024 * 1024;
const CORRECTION_WINDOW_MS = 5 * 60 * 1000;  // 5 minutes
const CLEANUP_AGE_MS = 10 * 60 * 1000;       // 10 minutes
const MAX_STR_LEN = 200;

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

function processToolUse() {
  let input;
  try {
    input = JSON.parse(data);
  } catch {
    return;
  }

  // Support both old_string/new_string and old_str/new_str field names
  const filePath = input.tool_input?.file_path || input.tool_input?.path || '';
  const oldStr = input.tool_input?.old_string || input.tool_input?.old_str || '';
  const newStr = input.tool_input?.new_string || input.tool_input?.new_str || '';

  if (!filePath || !oldStr) return;

  const cwdHash = crypto.createHash('sha256').update(process.cwd()).digest('hex').slice(0, 12);
  const trackFile = path.join('/tmp', `hyphae-recent-edits-${cwdHash}.json`);

  const recentEdits = loadAndCleanEdits(trackFile);
  const correctedEdit = findCorrection(filePath, oldStr, recentEdits);

  if (correctedEdit) {
    const hyphaeAvailable = commandExists('hyphae');
    if (hyphaeAvailable) {
      storeCorrectionInHyphae(filePath, correctedEdit, oldStr, newStr);
    }
  }

  // Track this edit for future correction detection
  recentEdits.push({
    file: filePath,
    old_string: truncate(oldStr),
    new_string: truncate(newStr),
    timestamp: Date.now()
  });
  saveEdits(trackFile, recentEdits);
}

function truncate(str) {
  if (str.length <= MAX_STR_LEN) return str;
  return str.slice(0, MAX_STR_LEN) + '...';
}

function loadAndCleanEdits(trackFile) {
  let edits = [];
  try {
    if (fs.existsSync(trackFile)) {
      edits = JSON.parse(fs.readFileSync(trackFile, 'utf8'));
      if (!Array.isArray(edits)) edits = [];
    }
  } catch {
    edits = [];
  }

  // Remove entries older than 10 minutes
  const cutoff = Date.now() - CLEANUP_AGE_MS;
  return edits.filter(e => e.timestamp > cutoff);
}

function saveEdits(trackFile, edits) {
  try {
    fs.writeFileSync(trackFile, JSON.stringify(edits, null, 2), 'utf8');
  } catch {
    // Non-critical
  }
}

function findCorrection(filePath, oldStr, recentEdits) {
  const cutoff = Date.now() - CORRECTION_WINDOW_MS;
  const candidates = recentEdits.filter(
    e => e.file === filePath && e.timestamp > cutoff && e.new_string
  );

  for (const prev of candidates) {
    // Correction: current old_string overlaps previous new_string
    // (agent is undoing or fixing what it just wrote)
    if (prev.new_string.includes(oldStr) || oldStr.includes(prev.new_string)) {
      return prev;
    }
  }
  return null;
}

function storeCorrectionInHyphae(filePath, correctedEdit, newOldStr, newNewStr) {
  try {
    const project = getProjectName();
    const fileName = path.basename(filePath);
    const content = [
      `File: ${fileName}`,
      `Original change: ${correctedEdit.old_string} → ${correctedEdit.new_string}`,
      `Correction: ${truncate(newOldStr)} → ${truncate(newNewStr)}`
    ].join('\n');

    const args = [
      'store', '--topic', 'corrections',
      '--content', content,
      '--importance', 'high',
      '--keywords', `correction,self-fix,${fileName}`
    ];
    if (project) args.push('-P', project);

    spawnSync('hyphae', args, {
      encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'], timeout: 3000
    });
    log(`[capture-corrections] Stored correction for ${fileName} in hyphae`);
  } catch {
    // Non-critical
  }
}
