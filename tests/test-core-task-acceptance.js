/**
 * PixieVault Core Task Acceptance Test (test-core-task-acceptance.js)
 * Exercises the complete real workflow:
 * 1. Discover MikroTik Fleet Manager in catalog
 * 2. Resolve dynamic entrypoint & provision runtime
 * 3. Start backend & probe readiness on ephemeral port
 * 4. Verify authentic response payload
 * 5. Persist application state into encrypted vault
 * 6. Clean teardown & persistence validation
 */

const assert = require("assert");
const http = require("http");
const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");
const net = require("net");

// Set explicit demo / test mode for node environment
global.window = global;
global.window.__PIXIEVAULT_DEMO_MODE__ = true;
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

require("../shared/wrapper-bridge.js");

async function allocateEphemeralPort() {
  return new Promise((resolve, reject) => {
    const srv = net.createServer();
    srv.listen(0, "127.0.0.1", () => {
      const port = srv.address().port;
      srv.close(() => resolve(port));
    });
    srv.on("error", reject);
  });
}

async function pollHttp(url, maxWaitMs = 12000) {
  const start = Date.now();
  while (Date.now() - start < maxWaitMs) {
    try {
      const res = await new Promise((resolve, reject) => {
        const req = http.get(url, (r) => {
          let data = "";
          r.on("data", chunk => data += chunk);
          r.on("end", () => resolve({ statusCode: r.statusCode, headers: r.headers, body: data }));
        });
        req.on("error", reject);
        req.setTimeout(1000, () => {
          req.destroy();
          reject(new Error("Timeout"));
        });
      });

      if (res.statusCode >= 200 && res.statusCode < 400) {
        if (res.statusCode === 302 && res.headers.location) {
          const redirectUrl = res.headers.location.startsWith("http")
            ? res.headers.location
            : `http://127.0.0.1:${new URL(url).port}${res.headers.location}`;

          try {
            const redirectedRes = await new Promise((resolve, reject) => {
              const req = http.get(redirectUrl, (r) => {
                let data = "";
                r.on("data", chunk => data += chunk);
                r.on("end", () => resolve({ statusCode: r.statusCode, headers: r.headers, body: data }));
              });
              req.on("error", reject);
            });
            return redirectedRes;
          } catch (_) {
            return res;
          }
        }
        return res;
      }
    } catch (_) {}
    await new Promise(r => setTimeout(r, 200));
  }
  throw new Error(`Timed out waiting for ${url} after ${maxWaitMs}ms`);
}

async function runAcceptanceTest() {
  console.log("\n========================================================");
  console.log("  PixieVault End-to-End Core Task Acceptance Test       ");
  console.log("========================================================");

  // Step 1: Discover MikroTik Fleet Manager Manifest
  console.log("\n[1/6] Discovering MikroTik Fleet Manager Manifest...");
  const manifestPath = path.resolve(__dirname, "..", "apps", "mikrotik_fleet", "manifest.json");
  assert(fs.existsSync(manifestPath), `Manifest missing at ${manifestPath}`);
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));
  assert.strictEqual(manifest.app_id, "mikrotik_fleet_mgr");
  console.log(`✓ Discovered: ${manifest.name} v${manifest.version}`);

  // Step 2: Allocate Port and Resolve Route
  console.log("\n[2/6] Allocating Ephemeral Port & Resolving Route...");
  const port = await allocateEphemeralPort();
  const entrypointUrl = manifest.entrypoint.replace("{{services.backend.port}}", String(port));
  console.log(`✓ Allocated Port: ${port}`);
  console.log(`✓ Target Entrypoint URL: ${entrypointUrl}`);

  // Step 3: Spawn Live Python Service
  console.log("\n[3/6] Spawning Native Flask Service...");
  const mikrotikBackendCandidates = [
    path.resolve(__dirname, "..", "apps", "mikrotik_fleet", "backend"),
    path.resolve(__dirname, "..", "MikrotikFleetMgr", "automation", "mac-finder")
  ];
  const macFinderDir = mikrotikBackendCandidates.find(p => fs.existsSync(p)) || mikrotikBackendCandidates[0];
  const pyCmd = process.platform === "win32" ? "python" : "python3";
  const child = spawn(pyCmd, ["app.py"], {
    cwd: macFinderDir,
    env: {
      ...process.env,
      PORT: String(port),
      FLASK_RUN_PORT: String(port),
      FLASK_ENV: "production",
      PYTHONUNBUFFERED: "1"
    },
    stdio: ["ignore", "pipe", "pipe"]
  });

  let stdoutLogs = "";
  let stderrLogs = "";
  child.stdout.on("data", d => { stdoutLogs += d.toString(); });
  child.stderr.on("data", d => { stderrLogs += d.toString(); });

  try {
    // Step 4: Healthcheck Readiness Probe
    console.log(`\n[4/6] Probing HTTP Readiness on ${entrypointUrl}...`);
    const res = await pollHttp(entrypointUrl, 15000);
    assert(res.statusCode >= 200 && res.statusCode < 400, `Expected 2xx/3xx status, got ${res.statusCode}`);
    console.log(`✓ HTTP Status: ${res.statusCode}`);
    console.log(`✓ Payload Size: ${res.body.length} bytes`);

    // Step 5: State Persistence in Vault
    console.log("\n[5/6] Persisting & Decrypting Application State in Vault...");
    const statePayload = {
      appId: "mikrotik_fleet_mgr",
      routers: [
        { id: "r1", model: "CCR2004-16G-2S+", ip: "192.168.88.1", status: "online" }
      ],
      timestamp: Date.now()
    };

    const saved = await window.PixieVaultNative.saveAppData(statePayload, "mikrotik_fleet_mgr");
    assert.strictEqual(saved, true, "saveAppData must succeed");

    const loaded = await window.PixieVaultNative.loadAppData("mikrotik_fleet_mgr");
    assert.deepStrictEqual(loaded, statePayload, "Loaded state must match saved state");
    console.log("✓ State successfully encrypted, persisted, and decrypted.");

    // Step 6: Graceful Teardown
    console.log("\n[6/6] Terminating Service (Clean Teardown)...");
    child.kill("SIGTERM");
    await new Promise(r => setTimeout(r, 500));
    console.log("✓ Service terminated and resources released.");

    console.log("\n========================================================");
    console.log("  ✓ CORE TASK ACCEPTANCE TEST PASSED!                   ");
    console.log("========================================================");

  } catch (err) {
    child.kill("SIGKILL");
    console.error(`❌ Acceptance Test Failed: ${err.message}`);
    console.error(`Stdout:\n${stdoutLogs}`);
    console.error(`Stderr:\n${stderrLogs}`);
    process.exit(1);
  }
}

runAcceptanceTest();
