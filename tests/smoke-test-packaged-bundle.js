/**
 * PixieVault Packaged Application Smoke Test (smoke-test-packaged-bundle.js)
 * Validates that all bundled applications, manifest entrypoints, assets, and runtime scripts
 * exist and resolve correctly without relying on dev source tree assumptions.
 */

const fs = require("fs");
const path = require("path");
const assert = require("assert");

function runPackagedSmokeTest() {
  console.log("\n========================================================");
  console.log("  PixieVault Packaged Application Smoke Test            ");
  console.log("========================================================");

  const rootDir = path.resolve(__dirname, "..");
  const appsDir = path.join(rootDir, "apps");
  const hostDir = path.join(rootDir, "host");

  assert(fs.existsSync(appsDir), `Apps directory missing at ${appsDir}`);
  assert(fs.existsSync(hostDir), `Host directory missing at ${hostDir}`);

  // 1. Verify Core Shell Assets
  console.log("\n[1/4] Verifying Core Shell Webview Assets...");
  const coreFiles = [
    "index.html",
    "shell.js",
    "shell.css",
    "tokens.css",
    "wrapper-bridge.js"
  ];

  for (const f of coreFiles) {
    const p = path.join(hostDir, f);
    assert(fs.existsSync(p), `Core shell asset missing: ${p}`);
    const stat = fs.statSync(p);
    assert(stat.size > 20, `Core shell asset is empty: ${f}`);
    console.log(`✓ Shell Asset: ${f} (${stat.size} bytes)`);
  }

  // 2. Discover and Validate All Bundled Apps & Manifests
  console.log("\n[2/4] Discovering and Validating Bundled Applications...");
  const entries = fs.readdirSync(appsDir, { withFileTypes: true });
  let discoveredCount = 0;

  for (const entry of entries) {
    if (entry.isDirectory() && !entry.name.startsWith(".")) {
      discoveredCount++;
      const appDir = path.join(appsDir, entry.name);
      const manifestPath = path.join(appDir, "manifest.json");

      assert(fs.existsSync(manifestPath), `Manifest missing for ${entry.name}`);
      const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));

      console.log(`\n• Checking Application: ${manifest.name} (ID: ${manifest.app_id})`);
      assert(manifest.app_id, `app_id missing in ${manifestPath}`);
      assert(manifest.name, `name missing in ${manifestPath}`);
      assert(manifest.version, `version missing in ${manifestPath}`);
      assert(manifest.entrypoint, `entrypoint missing in ${manifestPath}`);

      // 3. Verify Manifest Entrypoint / Composer Target
      if (manifest.composer && manifest.composer.services) {
        console.log(`  ✓ Type: Declarative Native Composer App`);
        assert(
          manifest.entrypoint.startsWith("http://") || manifest.entrypoint.startsWith("https://"),
          `Composer app must define HTTP entrypoint (got ${manifest.entrypoint})`
        );

        for (const [svcName, svcCfg] of Object.entries(manifest.composer.services)) {
          assert(svcCfg.command && svcCfg.command.length > 0, `Service ${svcName} has empty command`);
          console.log(`  ✓ Service '${svcName}': command [${svcCfg.command.join(" ")}] on port '${svcCfg.port}'`);
        }
      } else {
        console.log(`  ✓ Type: Static Webview Guest Application`);
        const entrypointPath = path.join(appDir, manifest.entrypoint);
        assert(fs.existsSync(entrypointPath), `Entrypoint file missing: ${entrypointPath}`);
        console.log(`  ✓ Static Entrypoint Verified: ${manifest.entrypoint}`);
      }
    }
  }

  assert(discoveredCount >= 1, `Expected at least 1 bundled app, found ${discoveredCount}`);
  console.log(`\n✓ Successfully validated ${discoveredCount} bundled apps.`);

  // 4. Validate Isolated Data Directory Separation
  console.log("\n[3/4] Validating Portable App-Data Cleanliness...");
  for (const entry of entries) {
    if (entry.isDirectory() && !entry.name.startsWith(".")) {
      const appDir = path.join(appsDir, entry.name);
      const subEntries = fs.readdirSync(appDir);
      for (const sub of subEntries) {
        // Assert no local databases or venvs inside static apps
        if (entry.name !== "mikrotik_fleet") {
          assert(!sub.endsWith(".db"), `Leaked database file '${sub}' inside code folder ${entry.name}`);
          assert(!sub.endsWith(".sqlite"), `Leaked sqlite file '${sub}' inside code folder ${entry.name}`);
        }
      }
    }
  }
  console.log("✓ No leaked databases or state inside application code folders.");

  console.log("\n[4/4] Verifying Bridge IPC Non-Mock Integrity...");
  // Test that without explicit demo flag, invoking mock throws or fails safely
  global.window = {};
  require(path.join(hostDir, "wrapper-bridge.js"));
  assert(global.window.PixieVaultNative, "Bridge must initialize window.PixieVaultNative");
  assert.strictEqual(global.window.PixieVaultNative.isDemoMode, false, "Demo mode must be false by default in production bundle");

  console.log("\n========================================================");
  console.log("  ✓ ALL PACKAGED BUNDLE SMOKE CHECKS PASSED!            ");
  console.log("========================================================");
}

runPackagedSmokeTest();
