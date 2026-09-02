#!/usr/bin/env node
/**
 * PixieVault Bridge Synchronization Script
 * Copies the canonical shared/wrapper-bridge.js to all target distribution locations.
 */

const fs = require("fs");
const path = require("path");

const rootDir = path.resolve(__dirname, "..");
const canonicalBridge = path.join(rootDir, "shared", "wrapper-bridge.js");

if (!fs.existsSync(canonicalBridge)) {
  console.error(`❌ Canonical bridge not found: ${canonicalBridge}`);
  process.exit(1);
}

const targets = [
  path.join(rootDir, "wrapper-bridge.js"),
  path.join(rootDir, "host", "wrapper-bridge.js")
];

const content = fs.readFileSync(canonicalBridge, "utf-8");

console.log("========================================");
console.log("  PixieVault Bridge Sync Utility        ");
console.log("========================================");
console.log(`Source: ${canonicalBridge} (${content.length} bytes)`);

let synced = 0;
for (const target of targets) {
  const dir = path.dirname(target);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
  fs.writeFileSync(target, content, "utf-8");
  console.log(`✓ Synchronized -> ${target}`);
  synced++;
}

console.log(`\n✓ Successfully synchronized ${synced} bridge target(s).\n`);
