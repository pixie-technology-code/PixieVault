/**
 * PixieVault Automated Frontend Bridge & Shell Test Suite
 * Executes automated test cases against IPC bridge, storage roundtrip, and inter-app telemetry.
 */

const assert = require("assert");
const fs = require("fs");
const path = require("path");

// Setup browser-like environment in Node
global.window = global;
global.window.__PIXIEVAULT_DEMO_MODE__ = true;
const demoCatalogPath = path.join(__dirname, "../host/demo-catalog.json");
if (fs.existsSync(demoCatalogPath)) {
  global.window.__PIXIEVAULT_DEMO_CATALOG__ = JSON.parse(fs.readFileSync(demoCatalogPath, "utf-8"));
}
global.localStorage = {
  _store: {},
  getItem(k) { return this._store[k] || null; },
  setItem(k, v) { this._store[k] = String(v); },
  removeItem(k) { delete this._store[k]; },
  clear() { this._store = {}; }
};
global.addEventListener = () => {};
global.document = {
  addEventListener: () => {},
  getElementById: () => ({ innerText: "", innerHTML: "", style: {}, value: "", classList: { add() {}, remove() {}, contains() { return false; } } }),
  querySelectorAll: () => []
};

// Load wrapper-bridge.js
require("../shared/wrapper-bridge.js");

async function runBridgeTestSuite() {
  console.log("\n========================================");
  console.log("  PixieVault Frontend & Bridge Tests    ");
  console.log("========================================");

  let passed = 0;
  let total = 0;

  async function test(name, fn) {
    total++;
    try {
      await fn();
      console.log(`✓ PASS: ${name}`);
      passed++;
    } catch (err) {
      console.error(`❌ FAIL: ${name}\n   Error: ${err.message}`);
    }
  }

  // Test 1: Bridge Availability
  await test("PixieVaultNative object is initialized", async () => {
    assert(window.PixieVaultNative, "window.PixieVaultNative is undefined");
    assert(typeof window.PixieVaultNative.getVaultStatus === "function");
    assert(typeof window.PixieVaultNative.authenticateBiometrics === "function");
    assert(typeof window.PixieVaultNative.authenticatePassword === "function");
    assert(typeof window.PixieVaultNative.loadAppData === "function");
    assert(typeof window.PixieVaultNative.saveAppData === "function");
  });

  // Test 2: Vault Status Query
  await test("getVaultStatus() returns valid structure", async () => {
    const status = await window.PixieVaultNative.getVaultStatus();
    assert(typeof status === "object");
    assert(status !== null);
    assert("is_locked" in status);
    assert("biometrics_available" in status);
  });

  // Test 3: Biometric Authentication Flow
  await test("authenticateBiometrics() unlocks vault", async () => {
    const auth = await window.PixieVaultNative.authenticateBiometrics();
    assert.strictEqual(auth.success, true);
  });

  // Test 4: Master Passphrase Authentication Flow
  await test("authenticatePassword() unlocks vault with password", async () => {
    const auth = await window.PixieVaultNative.authenticatePassword("customPassword123");
    assert.strictEqual(auth.success, true);
  });

  // Test 5: State Persistence Roundtrip
  await test("saveAppData() and loadAppData() roundtrip persistence", async () => {
    const testPayload = {
      netWorth: 1850000,
      portfolioBalance: 1420000,
      customNote: "Cairn Wealth Navigation Plan 2026",
      timestamp: Date.now()
    };

    const saved = await window.PixieVaultNative.saveAppData(testPayload, "cairn_dead_reckoning");
    assert.strictEqual(saved, true);

    const loaded = await window.PixieVaultNative.loadAppData("cairn_dead_reckoning");
    assert.deepStrictEqual(loaded, testPayload);
  });

  // Test 6: App Catalog Listing
  await test("listInstalledApps() returns discovered applications", async () => {
    const apps = await window.PixieVaultNative.listInstalledApps();
    assert(Array.isArray(apps));
    assert(apps.length >= 2, "Expected at least 2 installed apps");
    
    const appIds = apps.map(a => a.manifest.app_id);
    assert(appIds.includes("mikrotik_fleet_mgr"));
    assert(appIds.includes("cairn_dead_reckoning"));
  });

  // Test 7: Inter-App Brokered Bus Metric Queries
  await test("requestCrossAppData() queries adjacent app metrics", async () => {
    const netWorth = await window.PixieVaultNative.requestCrossAppData("cairn_dead_reckoning", "netWorth", "mikrotik_fleet_mgr");
    assert.strictEqual(netWorth, 1850000);

    const onlineDevices = await window.PixieVaultNative.requestCrossAppData("mikrotik_fleet_mgr", "onlineDevices", "cairn_dead_reckoning");
    assert.strictEqual(onlineDevices, 12);
  });

  // Test 8: Lock Vault Purge
  await test("lockVault() locks state", async () => {
    const locked = await window.PixieVaultNative.lockVault();
    assert.strictEqual(locked, true);
  });

  // Test 9: MikroTik Fleet Manager State Persistence
  await test("MikroTik Fleet Manager state persistence in vault", async () => {
    const fleetPayload = {
      devices: [
        { name: "core-router-01", model: "CCR2004-16G-2S+", ip: "192.168.88.1", status: "online" }
      ],
      updatedAt: Date.now()
    };
    const saved = await window.PixieVaultNative.saveAppData(fleetPayload, "mikrotik_fleet_mgr");
    assert.strictEqual(saved, true);

    const loaded = await window.PixieVaultNative.loadAppData("mikrotik_fleet_mgr");
    assert.strictEqual(loaded.devices[0].name, "core-router-01");
  });

  // Test 10: Native Composer APIs availability & mocking
  await test("startComposerApp() and stopComposerApp() lifecycle", async () => {
    assert(typeof window.PixieVaultNative.startComposerApp === "function");
    assert(typeof window.PixieVaultNative.stopComposerApp === "function");
    assert(typeof window.PixieVaultNative.getComposerStatus === "function");

    const status = await window.PixieVaultNative.startComposerApp("mikrotik_fleet_mgr");
    assert(status !== null);
    assert.strictEqual(status.is_running, true);

    const stopped = await window.PixieVaultNative.stopComposerApp("mikrotik_fleet_mgr");
    assert.strictEqual(stopped, true);
  });

  console.log(`\nResults: ${passed}/${total} Frontend Bridge Tests Passed.`);
  if (passed !== total) {
    process.exit(1);
  }
}

runBridgeTestSuite().catch(err => {
  console.error("Fatal test runner error:", err);
  process.exit(1);
});
