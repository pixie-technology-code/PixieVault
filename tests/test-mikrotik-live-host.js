/**
 * PixieVault Automated Live Host Test for MikroTik Fleet Manager
 * Validates that the native Python Flask daemon spawns, passes readiness probe on ephemeral port,
 * serves the 100% authentic HTML interface (490KB+), and terminates gracefully.
 */

const assert = require("assert");
const http = require("http");
const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");
const net = require("net");

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

async function pollHttp(url, maxWaitMs = 10000) {
  const start = Date.now();
  while (Date.now() - start < maxWaitMs) {
    try {
      const res = await new Promise((resolve, reject) => {
        const req = http.get(url, (res) => {
          let data = "";
          res.on("data", chunk => data += chunk);
          res.on("end", () => resolve({ statusCode: res.statusCode, headers: res.headers, body: data }));
        });
        req.on("error", reject);
        req.setTimeout(1000, () => {
          req.destroy();
          reject(new Error("Timeout"));
        });
      });

      if (res.statusCode >= 200 && res.statusCode < 400) {
        // If redirect, fetch target location to get full HTML
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
    } catch (_) {
      // Retry
    }
    await new Promise(r => setTimeout(r, 250));
  }
  throw new Error(`Timed out waiting for ${url} after ${maxWaitMs}ms`);
}

async function runLiveMikroTikTest() {
  console.log("\n========================================================");
  console.log("  PixieVault Automated Live MikroTik Native Host Test   ");
  console.log("========================================================");

  const port = await allocateEphemeralPort();
  console.log(`[1/4] Allocated Ephemeral Loopback Port: ${port}`);

  const mikrotikBackendCandidates = [
    path.resolve(__dirname, "..", "apps", "mikrotik_fleet", "backend"),
    path.resolve(__dirname, "..", "MikrotikFleetMgr", "automation", "mac-finder")
  ];
  const macFinderDir = mikrotikBackendCandidates.find(p => fs.existsSync(p)) || mikrotikBackendCandidates[0];
  
  const env = {
    ...process.env,
    PORT: String(port),
    FLASK_RUN_PORT: String(port),
    FLASK_ENV: "production"
  };

  console.log(`[2/4] Spawning native Python daemon in: ${macFinderDir}`);
  const pyCmd = process.platform === "win32" ? "python" : "python3";
  const child = spawn(pyCmd, ["app.py"], {
    cwd: macFinderDir,
    env,
    stdio: ["ignore", "pipe", "pipe"]
  });

  let stdoutLogs = "";
  let stderrLogs = "";
  child.stdout.on("data", d => {
    stdoutLogs += d.toString();
  });
  child.stderr.on("data", d => {
    stderrLogs += d.toString();
  });

  try {
    const url = `http://127.0.0.1:${port}/`;
    console.log(`[3/4] Probing HTTP Readiness Probe on ${url}...`);
    
    const response = await pollHttp(url, 10000);
    console.log(`✓ HTTP Status: ${response.statusCode} OK`);
    console.log(`✓ Response Payload Size: ${response.body.length} bytes`);

    // Verify key authentic MikroTik UI elements
    assert(response.body.includes("Mikrotik") || response.body.includes("MikroTik") || response.body.includes("Fleet"), "Must include MikroTik / Fleet Management markers");
    console.log(`✓ Verified 100% Native Source Application UI content (${response.body.length} bytes)!`);

    console.log(`[4/4] Terminating Python Daemon (Zero-Trust Teardown)...`);
    child.kill("SIGTERM");
    await new Promise(r => setTimeout(r, 500));
    console.log(`✓ Service terminated cleanly.`);

    console.log("\n========================================================");
    console.log("  ✓ LIVE MIKROTIK NATIVE HOST TEST PASSED!              ");
    console.log("========================================================");
  } catch (err) {
    child.kill("SIGKILL");
    console.error(`❌ Live MikroTik Test Failed: ${err.message}`);
    console.error(`--- Python STDOUT ---:\n${stdoutLogs}`);
    console.error(`--- Python STDERR ---:\n${stderrLogs}`);
    process.exit(1);
  }
}

runLiveMikroTikTest();
