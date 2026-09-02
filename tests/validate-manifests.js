/**
 * PixieVault Automated Manifest & Ecosystem Validator
 * Validates all installed app manifests against the schema (supporting static and Composer apps).
 */

const fs = require("fs");
const path = require("path");

const APPS_DIR = path.join(__dirname, "..", "apps");

function validateManifests() {
  console.log("\n========================================");
  console.log("  PixieVault App Manifest Validation     ");
  console.log("========================================");

  let totalApps = 0;
  let errors = 0;

  const entries = fs.readdirSync(APPS_DIR, { withFileTypes: true });

  for (const entry of entries) {
    if (entry.isDirectory() && !entry.name.startsWith(".")) {
      const manifestPath = path.join(APPS_DIR, entry.name, "manifest.json");
      totalApps++;

      if (!fs.existsSync(manifestPath)) {
        console.error(`❌ [${entry.name}] Missing manifest.json`);
        errors++;
        continue;
      }

      try {
        const raw = fs.readFileSync(manifestPath, "utf-8");
        const json = JSON.parse(raw);

        // Required fields
        if (!json.app_id || typeof json.app_id !== "string") {
          throw new Error("Missing or invalid 'app_id'");
        }
        if (!json.name || typeof json.name !== "string") {
          throw new Error("Missing or invalid 'name'");
        }
        if (!json.version || typeof json.version !== "string") {
          throw new Error("Missing or invalid 'version'");
        }
        if (!json.min_pixievault_version || typeof json.min_pixievault_version !== "string") {
          throw new Error("Missing or invalid 'min_pixievault_version'");
        }
        const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;
        if (!semver.test(json.min_pixievault_version)) {
          throw new Error("'min_pixievault_version' must be valid SemVer without a leading 'v'");
        }
        if (!json.entrypoint || typeof json.entrypoint !== "string") {
          throw new Error("Missing or invalid 'entrypoint'");
        }

        // Verify entrypoint (dynamic Composer HTTP URL or local static file)
        if (json.entrypoint.startsWith("http://") || json.entrypoint.startsWith("https://")) {
          if (!json.composer || !json.composer.services || Object.keys(json.composer.services).length === 0) {
            throw new Error(`Dynamic HTTP entrypoint '${json.entrypoint}' requires 'composer.services'`);
          }
          console.log(`✓ [${json.app_id}] "${json.name}" v${json.version} — Valid Composer Manifest (${json.entrypoint})`);
        } else {
          const entrypointPath = path.join(APPS_DIR, entry.name, json.entrypoint);
          if (!fs.existsSync(entrypointPath)) {
            throw new Error(`Entrypoint '${json.entrypoint}' file not found at ${entrypointPath}`);
          }
          console.log(`✓ [${json.app_id}] "${json.name}" v${json.version} — Valid Static Manifest & Entrypoint`);
        }

      } catch (err) {
        console.error(`❌ [${entry.name}] Manifest Validation Error: ${err.message}`);
        errors++;
      }
    }
  }

  console.log(`\nResults: ${totalApps - errors}/${totalApps} App Manifests Valid.`);
  if (errors > 0) {
    process.exit(1);
  }
}

validateManifests();
