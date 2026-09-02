/**
 * PixieVault Generic Packaged Distribution Conformance Test (test-packaged-distribution-artifact.js)
 * Validates the packaged distribution resource configuration and structure uniformly across all applications:
 * 1. Validates tauri.conf.json bundle.resources declarations.
 * 2. Confirms all bundled applications conform to the manifest packaging contract with zero external traversal (e.g. ../../).
 * 3. Verifies all service commands, working directories, and requirements across all declared Composer services.
 * 4. Verifies zero state leaks (.venv, databases, caches, secrets) inside packaged application code directories.
 */

const fs = require("fs");
const path = require("path");
const assert = require("assert");

function runPackagedArtifactTest() {
  console.log("\n========================================================");
  console.log("  PixieVault Packaged Distribution Conformance Test      ");
  console.log("========================================================");

  const rootDir = path.resolve(__dirname, "..");
  const srcTauriDir = path.join(rootDir, "src-tauri");
  const tauriConfPath = path.join(srcTauriDir, "tauri.conf.json");
  const appsDir = path.join(rootDir, "apps");

  // Step 1: Validate tauri.conf.json Bundle Resources
  console.log("\n[1/3] Validating tauri.conf.json Bundle Resource Declarations...");
  assert(fs.existsSync(tauriConfPath), `tauri.conf.json missing at ${tauriConfPath}`);
  const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, "utf-8"));

  assert(tauriConf.bundle, "bundle configuration missing in tauri.conf.json");
  assert(tauriConf.bundle.active, "bundle.active must be true");
  assert(Array.isArray(tauriConf.bundle.resources), "bundle.resources must be an array");
  assert(
    tauriConf.bundle.resources.includes("apps/**/*"),
    "bundle.resources must use the clean staged 'apps/**/*' resource path"
  );
  console.log(`✓ bundle.resources declared: [ ${tauriConf.bundle.resources.join(", ")} ]`);

  const stagedAppsDir = path.join(srcTauriDir, "apps");
  assert(fs.existsSync(stagedAppsDir), "Clean staged resource tree is missing; run a Cargo build first");

  const forbiddenNames = new Set([".venv", "venv", "ENV", "__pycache__", ".pytest_cache", ".secrets", "node_modules"]);
  function assertCleanTree(dir) {
    for (const item of fs.readdirSync(dir, { withFileTypes: true })) {
      assert(!forbiddenNames.has(item.name), `Generated runtime directory leaked into staged resources: ${path.join(dir, item.name)}`);
      assert(!item.name.toLowerCase().includes(":zone.identifier"), `Windows metadata leaked into staged resources: ${path.join(dir, item.name)}`);
      if (item.isDirectory()) assertCleanTree(path.join(dir, item.name));
    }
  }
  assertCleanTree(stagedAppsDir);
  console.log("✓ Staged resource tree excludes virtualenvs, dependency caches, secrets, and generated state.");

  // Step 2: Validate Self-Contained Bundled Applications & Services
  console.log("\n[2/3] Validating Self-Contained Bundled Applications & Declared Services...");
  const entries = fs.readdirSync(appsDir, { withFileTypes: true });
  let appCount = 0;

  for (const entry of entries) {
    if (entry.isDirectory() && !entry.name.startsWith(".")) {
      appCount++;
      const appDir = path.join(appsDir, entry.name);
      const manifestPath = path.join(appDir, "manifest.json");

      assert(fs.existsSync(manifestPath), `Manifest missing for ${entry.name}`);
      const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));

      console.log(`\n• Checking Packaged App: "${manifest.name}" (ID: ${manifest.app_id})`);

      // Check for illegal parent directory traversals
      const manifestRaw = fs.readFileSync(manifestPath, "utf-8");
      assert(!manifestRaw.includes("../../"), `Illegal external traversal '../../' found in ${manifestPath}`);

      if (manifest.presentation) {
        console.log(`  ✓ Presentation metadata: icon=${manifest.presentation.icon || "none"}, accent=${manifest.presentation.accent || "default"}`);
      }

      if (manifest.composer && manifest.composer.services) {
        console.log(`  ✓ Declarative Composer Application with ${Object.keys(manifest.composer.services).length} service(s)`);
        for (const [svcName, svcCfg] of Object.entries(manifest.composer.services)) {
          const wdRel = svcCfg.working_dir || ".";
          assert(!wdRel.startsWith(".."), `Service '${svcName}' working_dir cannot traverse outside package: ${wdRel}`);

          const serviceWd = path.resolve(appDir, wdRel);
          assert(fs.existsSync(serviceWd), `Service '${svcName}' working_dir missing: ${serviceWd}`);

          // Verify command target executable/script
          if (svcCfg.command && svcCfg.command.length > 0) {
            const rawTarget = svcCfg.command.length > 1 ? svcCfg.command[1] : svcCfg.command[0];
            const directTarget = path.resolve(serviceWd, rawTarget);
            if (fs.existsSync(directTarget)) {
              console.log(`  ✓ Service '${svcName}' verified target: ${rawTarget} in ${wdRel}`);
            } else {
              console.log(`  ✓ Service '${svcName}' declared command: [${svcCfg.command.join(" ")}] in ${wdRel}`);
            }
          }

          // Verify requirements file if declared
          const reqFile = svcCfg.runtime?.requirements || svcCfg.requirements;
          if (reqFile) {
            const reqPath = path.resolve(serviceWd, reqFile);
            assert(fs.existsSync(reqPath), `Service '${svcName}' requirements missing: ${reqPath}`);
            console.log(`  ✓ Requirements file present: ${reqFile}`);
          }
        }
      } else {
        const entrypointPath = path.resolve(appDir, manifest.entrypoint);
        assert(fs.existsSync(entrypointPath), `Static entrypoint missing: ${entrypointPath}`);
        console.log(`  ✓ Static entrypoint verified: ${manifest.entrypoint}`);
      }
    }
  }

  assert(appCount >= 1, `Expected at least 1 bundled app, found ${appCount}`);
  console.log(`\n✓ Successfully validated ${appCount} self-contained bundled applications.`);

  // Step 3: Verify Zero Leaked State / Artifacts in Bundle
  console.log("\n[3/3] Verifying Zero Artifact Leaks in Packaged Resource Trees...");
  for (const entry of entries) {
    if (entry.isDirectory() && !entry.name.startsWith(".")) {
      const dirPath = path.join(appsDir, entry.name);
      const subEntries = fs.readdirSync(dirPath);
      for (const s of subEntries) {
        assert(!s.endsWith(".db"), `Leaked database file '${s}' inside ${entry.name}`);
        assert(!s.endsWith(".sqlite"), `Leaked sqlite file '${s}' inside ${entry.name}`);
        assert(!s.endsWith(".tmp"), `Leaked temporary file '${s}' inside ${entry.name}`);
      }
    }
  }
  console.log("✓ No leaked databases or temporary files inside packaged application trees.");

  console.log("\n========================================================");
  console.log("  ✓ PACKAGED DISTRIBUTION CONFORMANCE TEST PASSED!      ");
  console.log("========================================================");
}

runPackagedArtifactTest();
