#!/usr/bin/env node
/**
 * PixieVault Zero-Touch App Packager CLI (package-app.js)
 * Inspects any local directory, auto-generates manifest if needed, and packages into a .pvpkg archive.
 * 
 * Usage:
 *   node scripts/package-app.js <target-folder> [--name "App Name"] [--id "app_id"] [--output "app.pvpkg"]
 */

const fs = require("fs");
const path = require("path");
const zlib = require("zlib");

function parseArgs() {
  const args = process.argv.slice(2);
  if (args.length === 0 || args.includes("--help") || args.includes("-h")) {
    console.log(`
PixieVault App Packager CLI

Usage:
  node scripts/package-app.js <target-folder> [options]

Options:
  --name <string>        Display name of the application
  --id <string>          Unique application ID (e.g. mikrotik_fleet_mgr)
  --version <string>     Application version (default: 1.0.0)
  --entrypoint <string>  HTML entrypoint relative to app root (default: index.html)
  --desc <string>        Brief description of the application
  --output <path>        Destination .pvpkg file path (default: <app_id>.pvpkg)
    `);
    process.exit(0);
  }

  const targetDir = path.resolve(args[0]);
  const options = {
    targetDir,
    name: null,
    id: null,
    version: "1.0.0",
    entrypoint: null,
    description: "Packaged PixieVault guest application",
    output: null
  };


  for (let i = 1; i < args.length; i++) {
    if (args[i] === "--name" && args[i + 1]) options.name = args[++i];
    else if (args[i] === "--id" && args[i + 1]) options.id = args[++i];
    else if (args[i] === "--version" && args[i + 1]) options.version = args[++i];
    else if (args[i] === "--entrypoint" && args[i + 1]) options.entrypoint = args[++i];
    else if (args[i] === "--desc" && args[i + 1]) options.description = args[++i];
    else if (args[i] === "--output" && args[i + 1]) options.output = path.resolve(args[++i]);
  }

  return options;
}

// Simple pure-Node zip builder (PKZip standard)
class SimpleZipBuilder {
  constructor() {
    this.entries = [];
  }

  addFile(zipPath, data) {
    const buffer = Buffer.isBuffer(data) ? data : Buffer.from(data, "utf-8");
    const compressed = zlib.deflateRawSync(buffer);
    const crc = this.calculateCrc32(buffer);

    this.entries.push({
      name: zipPath.replace(/\\/g, "/"),
      uncompressedSize: buffer.length,
      compressedSize: compressed.length,
      compressedData: compressed,
      crc: crc
    });
  }

  calculateCrc32(buf) {
    let crc = ~0;
    for (let i = 0; i < buf.length; i++) {
      crc = (crc >>> 8) ^ this.crcTable[(crc ^ buf[i]) & 0xff];
    }
    return (crc ^ -1) >>> 0;
  }

  get crcTable() {
    if (!this._table) {
      this._table = new Uint32Array(256);
      for (let i = 0; i < 256; i++) {
        let c = i;
        for (let k = 0; k < 8; k++) {
          c = ((c & 1) ? (0xedb88320 ^ (c >>> 1)) : (c >>> 1));
        }
        this._table[i] = c >>> 0;
      }
    }
    return this._table;
  }

  build() {
    const localHeaders = [];
    const centralEntries = [];
    let offset = 0;

    for (const entry of this.entries) {
      const nameBuf = Buffer.from(entry.name, "utf-8");
      
      // Local File Header
      const lfh = Buffer.alloc(30 + nameBuf.length);
      lfh.writeUInt32LE(0x04034b50, 0); // Signature
      lfh.writeUInt16LE(20, 4);         // Version needed
      lfh.writeUInt16LE(0, 6);          // Flags
      lfh.writeUInt16LE(8, 8);          // Compression method: Deflate
      lfh.writeUInt16LE(0, 10);         // Mod time
      lfh.writeUInt16LE(0, 12);         // Mod date
      lfh.writeUInt32LE(entry.crc, 14); // CRC32
      lfh.writeUInt32LE(entry.compressedSize, 18);
      lfh.writeUInt32LE(entry.uncompressedSize, 22);
      lfh.writeUInt16LE(nameBuf.length, 26);
      lfh.writeUInt16LE(0, 28);         // Extra field length
      nameBuf.copy(lfh, 30);

      localHeaders.push(lfh, entry.compressedData);

      // Central Directory Header
      const cdh = Buffer.alloc(46 + nameBuf.length);
      cdh.writeUInt32LE(0x02014b50, 0); // Signature
      cdh.writeUInt16LE(20, 4);         // Version made by
      cdh.writeUInt16LE(20, 6);         // Version needed
      cdh.writeUInt16LE(0, 8);          // Flags
      cdh.writeUInt16LE(8, 10);         // Compression: Deflate
      cdh.writeUInt16LE(0, 12);         // Mod time
      cdh.writeUInt16LE(0, 14);         // Mod date
      cdh.writeUInt32LE(entry.crc, 16);
      cdh.writeUInt32LE(entry.compressedSize, 20);
      cdh.writeUInt32LE(entry.uncompressedSize, 24);
      cdh.writeUInt16LE(nameBuf.length, 28);
      cdh.writeUInt16LE(0, 30);         // Extra field length
      cdh.writeUInt16LE(0, 32);         // Comment length
      cdh.writeUInt16LE(0, 34);         // Disk number start
      cdh.writeUInt16LE(0, 36);         // Internal attributes
      cdh.writeUInt32LE(0, 38);         // External attributes
      cdh.writeUInt32LE(offset, 42);    // Relative offset of local header
      nameBuf.copy(cdh, 46);

      centralEntries.push(cdh);
      offset += lfh.length + entry.compressedData.length;
    }

    const centralDirBuffer = Buffer.concat(centralEntries);

    // End of Central Directory Record
    const eocd = Buffer.alloc(22);
    eocd.writeUInt32LE(0x06054b50, 0);
    eocd.writeUInt16LE(0, 4);                          // Disk number
    eocd.writeUInt16LE(0, 6);                          // Disk with central dir
    eocd.writeUInt16LE(this.entries.length, 8);         // Total entries this disk
    eocd.writeUInt16LE(this.entries.length, 10);        // Total entries central dir
    eocd.writeUInt32LE(centralDirBuffer.length, 12);    // Size of central dir
    eocd.writeUInt32LE(offset, 16);                     // Offset of central dir
    eocd.writeUInt16LE(0, 20);                          // Comment length

    return Buffer.concat([...localHeaders, centralDirBuffer, eocd]);
  }
}

function scanFiles(dir, root = dir) {
  let results = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const n = entry.name.toLowerCase();
    // Exclude portable app-data artifacts (.venv, __pycache__, databases, git, secrets, etc.)
    if (
      n === ".git" || n === ".venv" || n === "venv" || n === "env" ||
      n === "__pycache__" || n === ".pytest_cache" || n === ".secrets" ||
      n === "node_modules" || n === "temp" ||
      n.endsWith(".pyc") || n.endsWith(".db") || n.endsWith(".sqlite") ||
      n.endsWith(".sqlite3") || n.endsWith(".tmp") ||
      n.endsWith(":zone.identifier") || n.endsWith(".zone.identifier")
    ) {
      continue;
    }

    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results = results.concat(scanFiles(fullPath, root));
    } else {
      results.push({
        fullPath,
        relPath: path.relative(root, fullPath)
      });
    }
  }
  return results;
}

function packageApp() {
  const options = parseArgs();

  console.log("========================================");
  console.log("  PixieVault Zero-Touch App Packager    ");
  console.log("========================================");
  console.log(`Inspecting source directory: ${options.targetDir}`);

  if (!fs.existsSync(options.targetDir)) {
    console.error(`❌ Target directory does not exist: ${options.targetDir}`);
    process.exit(1);
  }

  // 1. Detect or build manifest
  let manifest = null;
  const manifestPath = path.join(options.targetDir, "manifest.json");

  if (fs.existsSync(manifestPath)) {
    console.log("✓ Existing manifest.json detected");
    manifest = JSON.parse(fs.readFileSync(manifestPath, "utf-8"));
    if (!manifest.min_pixievault_version) {
      throw new Error("Existing manifest.json must declare min_pixievault_version");
    }
  } else {
    console.log("ℹ No manifest.json found — auto-generating zero-touch manifest...");
    const dirName = path.basename(options.targetDir);
    const appId = options.id || dirName.toLowerCase().replace(/[^a-z0-9_]/g, "_");
    const appName = options.name || dirName.replace(/[_-]/g, " ").replace(/\b\w/g, c => c.toUpperCase());

    manifest = {
      app_id: appId,
      name: appName,
      version: options.version,
      min_pixievault_version: "0.1.0",
      description: options.description,
      entrypoint: options.entrypoint || "index.html",
      permissions: {
        requested_read: ["*:*"],
        requested_write: []
      }
    };


    // Standard Python entrypoint discovery
    const pyCandidates = [
      "app.py",
      "main.py",
      "server.py"
    ];

    let detectedPy = null;
    for (const c of pyCandidates) {
      if (fs.existsSync(path.join(options.targetDir, c))) {
        detectedPy = c;
        break;
      }
    }

    const pkgJsonPath = path.join(options.targetDir, "package.json");

    if (detectedPy) {
      console.log(`✓ Auto-detected Python backend service: ${detectedPy}`);
      const reqPath = path.join(options.targetDir, path.dirname(detectedPy), "requirements.txt");
      const hasReq = fs.existsSync(reqPath);
      manifest.entrypoint = "http://127.0.0.1:{{services.backend.port}}";
      manifest.composer = {
        version: "1",
        services: {
          backend: {
            command: ["python3", path.basename(detectedPy)],
            working_dir: path.dirname(detectedPy).replace(/\\/g, "/") || ".",
            port: "auto",
            runtime: {
              type: "python",
              requirements: hasReq ? "requirements.txt" : undefined,
              fingerprint_files: hasReq ? ["requirements.txt"] : undefined
            },
            environment: {
              PORT: "{{port}}",
              FLASK_RUN_PORT: "{{port}}",
              FLASK_ENV: "production"
            },
            healthcheck: {
              endpoint: "/",
              interval_ms: 200,
              timeout_ms: 8000
            }
          }
        }
      };
    } else if (fs.existsSync(pkgJsonPath)) {
      try {
        const pkg = JSON.parse(fs.readFileSync(pkgJsonPath, "utf-8"));
        let mainScript = null;
        if (pkg.main && fs.existsSync(path.join(options.targetDir, pkg.main))) {
          mainScript = pkg.main;
        } else if (fs.existsSync(path.join(options.targetDir, "server.js"))) {
          mainScript = "server.js";
        } else if (fs.existsSync(path.join(options.targetDir, "index.js"))) {
          mainScript = "index.js";
        } else if (pkg.scripts && pkg.scripts.start) {
          const match = pkg.scripts.start.match(/([a-zA-Z0-9_\-\./]+\.js)/);
          if (match && fs.existsSync(path.join(options.targetDir, match[1]))) {
            mainScript = match[1];
          }
        }

        if (!mainScript && options.entrypoint && fs.existsSync(path.join(options.targetDir, options.entrypoint))) {
          mainScript = options.entrypoint;
        }

        if (mainScript) {
          console.log(`✓ Auto-detected Node.js backend service: ${mainScript}`);
          manifest.entrypoint = "http://127.0.0.1:{{services.backend.port}}";
          manifest.composer = {
            version: "1",
            services: {
              backend: {
                command: ["node", mainScript.replace(/\\/g, "/")],
                port: "auto",
                runtime: {
                  type: "node",
                  fingerprint_files: ["package.json", "package-lock.json"]
                },
                environment: {
                  PORT: "{{port}}",
                  NODE_ENV: "production"
                },
                healthcheck: {
                  endpoint: "/",
                  interval_ms: 200,
                  timeout_ms: 8000
                }
              }
            }
          };
        } else {
          console.warn("⚠️ Warning: package.json found but could not resolve a main Node.js script. Specify --entrypoint if needed.");
        }
      } catch (e) {
        console.warn("⚠️ Error parsing package.json:", e.message);
      }
    }
  }

  // Override manifest options if provided via CLI
  if (options.name) manifest.name = options.name;
  if (options.id) manifest.app_id = options.id;
  if (options.version) manifest.version = options.version;
  if (options.entrypoint) manifest.entrypoint = options.entrypoint;

  // 2. Scan and collect files
  const files = scanFiles(options.targetDir);
  console.log(`✓ Scanned ${files.length} files in application directory`);

  // Verify entrypoint exists or is dynamic template
  const isTemplateEntry = manifest.entrypoint.includes("{{") || manifest.entrypoint.startsWith("http");
  const hasEntrypoint = files.some(f => f.relPath.replace(/\\/g, "/") === manifest.entrypoint);
  if (!isTemplateEntry && !hasEntrypoint) {
    console.warn(`⚠️ Warning: Specified entrypoint '${manifest.entrypoint}' was not found in directory.`);
  }


  // 3. Create zip package
  const zip = new SimpleZipBuilder();

  // Add manifest.json at root
  zip.addFile("manifest.json", JSON.stringify(manifest, null, 2));

  // Add code files under code/
  for (const f of files) {
    if (f.relPath === "manifest.json") continue;
    const data = fs.readFileSync(f.fullPath);
    zip.addFile(`code/${f.relPath}`, data);
  }

  const pkgBuffer = zip.build();
  const outputFile = options.output || path.join(process.cwd(), `${manifest.app_id}.pvpkg`);

  const outDir = path.dirname(outputFile);
  if (!fs.existsSync(outDir)) {
    fs.mkdirSync(outDir, { recursive: true });
  }

  fs.writeFileSync(outputFile, pkgBuffer);

  console.log("\n========================================");
  console.log(`✓ Package Created: ${outputFile}`);
  console.log(`  App ID:      ${manifest.app_id}`);
  console.log(`  Name:        ${manifest.name}`);
  console.log(`  Version:     ${manifest.version}`);
  console.log(`  Entrypoint:  code/${manifest.entrypoint}`);
  console.log(`  Files:       ${files.length} assets bundled`);
  console.log(`  Size:        ${(pkgBuffer.length / 1024).toFixed(1)} KB`);
  console.log("========================================\n");
}

packageApp();
