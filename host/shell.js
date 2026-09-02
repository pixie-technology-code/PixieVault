/**
 * PixieVault Unified Native Host Shell Logic (shell.js)
 * Fully native event dispatcher, biometric authenticator, dynamic workspace router, and telemetry broker.
 */

let hostState = {
  isLocked: true,
  activeWorkspace: "dashboard",
  providerName: "System Auth",
  platform: "linux",
  platformLabel: "System Security Provider",
  installedApps: []
};

const THEMES = ["theme-slate", "theme-emerald", "theme-sunset", "theme-solar"];
let currentThemeIdx = 0;

/**
 * Direct Native Menu Handler called by Rust window.eval
 */
window.PixieVaultShell = {
  handleNativeMenu(eventId) {
    console.log("[Native Menu Received in JS]:", eventId);
    
    // File Menu Actions
    if (eventId === "file_open_app" || eventId === "file_close_app" || eventId === "apps_dashboard") {
      switchWorkspace("dashboard");
    } else if (eventId.startsWith("switch_app_")) {
      const targetAppId = eventId.replace("switch_app_", "");
      switchWorkspace(targetAppId);
    } else if (eventId === "file_save_vault") {
      saveActiveAppState();
    } else if (eventId === "file_lock_vault") {
      lockVaultNow();
    }
    // Apps & Distribution
    else if (eventId === "apps_install_package") {
      triggerNativePackagePicker();
    } else if (eventId === "apps_install_local" || eventId === "apps_install_folder") {
      triggerNativeFolderPicker();
    } else if (eventId === "apps_install_github") {
      openInstallAppModal();
    } else if (eventId === "apps_check_updates") {
      checkForAllUpdates();
    } else if (eventId === "data_export_package") {
      promptExportCurrentPackage();
    }
    // Security & Auth
    else if (eventId === "auth_biometric") {
      triggerBiometricAuth();
    } else if (eventId === "auth_password") {
      showLockScreen();
      const input = document.getElementById("master-passphrase-input");
      if (input) input.focus();
    }
    // Themes
    else if (eventId === "theme_slate") {
      applyThemeByName("Slate Dark");
    } else if (eventId === "theme_emerald") {
      applyThemeByName("Cyber Emerald");
    } else if (eventId === "theme_sunset") {
      applyThemeByName("Sunset Amber");
    } else if (eventId === "theme_solar") {
      applyThemeByName("Solar Light");
    }
    // Other
    else if (eventId === "data_bus") {
      switchWorkspace("dashboard");
    } else if (eventId === "apps_reload") {
      window.location.reload();
    }
  }
};

/**
 * Initialize Host Shell
 */
async function initHostShell() {
  console.log("PixieVault Host Shell Initializing...");

  // 1. Fetch initial host status and platform details
  try {
    const status = await window.PixieVaultNative.getVaultStatus();
    hostState.isLocked = status.is_locked;
    hostState.providerName = status.biometric_provider || status.biometric_type || "Biometrics";
    hostState.platform = status.platform || (navigator.userAgent.includes("Win") ? "windows" : navigator.userAgent.includes("Mac") ? "macos" : "linux");
    hostState.platformLabel = status.platform_label || (hostState.platform === "windows" ? "Windows 11 / Windows Hello" : hostState.platform === "macos" ? "macOS / Touch ID" : "Linux / PAM Keyring");

    applyPlatformBranding();
  } catch (err) {
    console.warn("Host status check fallback:", err);
    applyPlatformBranding();
  }

  // 2. Setup Native Menu & Keyboard Event Listeners
  setupMenuEventListeners();
  setupKeyboardShortcuts();
  setupIframeEventListeners();

  // 3. Register default exports to Rust Inter-App Bus
  registerAllAppExports();

  // 4. Discover installed applications
  await refreshInstalledAppsList();

  // 5. Show initial screen
  if (!hostState.isLocked) {
    unlockAndShowWorkspace(hostState.activeWorkspace);
  } else {
    showLockScreen();
  }
}

/**
 * Apply dynamic platform branding based on Linux, Windows, or macOS
 */
function applyPlatformBranding() {
  // 1. Biometric button text tailored to OS
  const btnText = document.getElementById("btn-biometric-text");
  if (btnText) {
    if (hostState.platform === "windows") {
      btnText.innerText = "Unlock with Windows Hello";
    } else if (hostState.platform === "macos") {
      btnText.innerText = "Unlock with Touch ID";
    } else {
      btnText.innerText = "Unlock with Linux PAM / Keyring";
    }
  }

  // 2. Status hint
  const authMsg = document.getElementById("auth-status-msg");
  if (authMsg) {
    authMsg.innerText = `${hostState.providerName} Ready`;
  }

  // 3. Header badge & dashboard descriptions
  const platformBadge = document.getElementById("platform-badge");
  if (platformBadge) {
    platformBadge.innerText = hostState.platform === "windows" ? "Windows / WebView2" : hostState.platform === "macos" ? "macOS / WebKit" : "Linux / GTK3";
  }

  const secPlatform = document.getElementById("sec-platform-val");
  if (secPlatform) {
    secPlatform.innerText = hostState.platformLabel;
  }

  const desc = document.getElementById("dashboard-platform-desc");
  if (desc) {
    desc.innerText = `Native ${hostState.platformLabel.split('/')[0].trim()} host runtime with isolated encrypted workspaces.`;
  }
}

/**
 * Register native menu and IPC event listeners
 */
function setupMenuEventListeners() {
  window.PixieVaultNative.onHostEvent("menu_event", (eventId) => {
    window.PixieVaultShell.handleNativeMenu(eventId);
  });
}

/**
 * Setup Global Keyboard Shortcuts
 */
function setupKeyboardShortcuts() {
  document.addEventListener("keydown", (e) => {
    const isCmdOrCtrl = e.ctrlKey || e.metaKey;

    // Ctrl+Shift+A -> Trigger Biometrics
    if (isCmdOrCtrl && e.shiftKey && e.key.toLowerCase() === "a") {
      e.preventDefault();
      triggerBiometricAuth();
    }
    // Ctrl+L -> Lock Vault
    if (isCmdOrCtrl && e.key.toLowerCase() === "l") {
      e.preventDefault();
      lockVaultNow();
    }
    // Ctrl+S -> Save Active Workspace
    if (isCmdOrCtrl && e.key.toLowerCase() === "s") {
      e.preventDefault();
      saveActiveAppState();
    }
    // Ctrl+E -> Export Portable Package (.pvpkg)
    if (isCmdOrCtrl && e.key.toLowerCase() === "e") {
      e.preventDefault();
      promptExportCurrentPackage();
    }
    // Ctrl+W -> Close Active App / Go to Dashboard
    if (isCmdOrCtrl && e.key.toLowerCase() === "w") {
      e.preventDefault();
      switchWorkspace("dashboard");
    }
  });
}

/**
 * Setup iframe load & error listener
 */
function setupIframeEventListeners() {
  const frame = document.getElementById("guest-app-frame");
  if (!frame) return;

  frame.addEventListener("load", () => {
    // Dismiss loading overlay when page has finished loading
    if (frame.src && frame.src !== "about:blank") {
      const loadingOverlay = document.getElementById("guest-loading-overlay");
      if (loadingOverlay) loadingOverlay.style.display = "none";
    }
  });
}

/**
 * Refresh list of installed apps and populate selector
 */
async function refreshInstalledAppsList() {
  try {
    const apps = await window.PixieVaultNative.listInstalledApps();
    hostState.installedApps = Array.isArray(apps) ? apps : [];
    populateWorkspaceDropdown();
  } catch (err) {
    console.warn("Failed to fetch installed apps:", err);
  }
}

/**
 * Populate Workspace Selector Dropdown dynamically
 */
function populateWorkspaceDropdown() {
  const select = document.getElementById("workspace-select");
  if (!select) return;

  let optionsHtml = `<option value="dashboard">Host Ecosystem Dashboard</option>`;

  for (const app of hostState.installedApps) {
    const manifest = app.manifest;
    optionsHtml += `<option value="${manifest.app_id}">${manifest.name}</option>`;
  }

  select.innerHTML = optionsHtml;
  select.value = hostState.activeWorkspace;
}

/**
 * Biometric Authentication Handler
 */
async function triggerBiometricAuth() {
  const authMsg = document.getElementById("auth-status-msg");
  if (authMsg) {
    authMsg.innerHTML = `<span style="color: var(--app-accent-color)">Authenticating with ${hostState.providerName}...</span>`;
  }

  try {
    const result = await window.PixieVaultNative.authenticateBiometrics();
    if (result && result.success) {
      hostState.isLocked = false;
      if (authMsg) authMsg.innerHTML = `<span style="color: var(--app-success-color)">✓ Authentication Successful</span>`;
      
      const targetApp = result.auto_launch_app || "dashboard";
      setTimeout(() => unlockAndShowWorkspace(targetApp), 200);
    } else {
      if (authMsg) authMsg.innerHTML = `<span style="color: var(--app-danger-color)">✗ ${result?.error || "Authentication cancelled"}</span>`;
    }
  } catch (err) {
    if (authMsg) authMsg.innerHTML = `<span style="color: var(--app-danger-color)">Error: ${err.message || err}</span>`;
  }
}

/**
 * Master Passphrase Authentication Handler
 */
async function handlePasswordAuth(event) {
  if (event) event.preventDefault();
  const input = document.getElementById("master-passphrase-input");
  const authMsg = document.getElementById("auth-status-msg");
  const passphrase = input ? input.value.trim() : "";

  if (!passphrase) {
    if (authMsg) authMsg.innerHTML = `<span style="color: var(--app-warning-color)">Please enter master passphrase</span>`;
    return;
  }

  if (authMsg) {
    authMsg.innerHTML = `<span style="color: var(--app-accent-color)">Deriving Argon2id keys & decrypting vault...</span>`;
  }

  try {
    const result = await window.PixieVaultNative.authenticatePassword(passphrase);
    if (result && result.success) {
      hostState.isLocked = false;
      if (input) input.value = "";
      if (authMsg) authMsg.innerHTML = `<span style="color: var(--app-success-color)">✓ Vault Decrypted Successfully</span>`;
      
      const targetApp = result.auto_launch_app || "dashboard";
      setTimeout(() => unlockAndShowWorkspace(targetApp), 200);
    } else {
      if (authMsg) authMsg.innerHTML = `<span style="color: var(--app-danger-color)">✗ ${result?.error || "Invalid passphrase"}</span>`;
    }
  } catch (err) {
    if (authMsg) authMsg.innerHTML = `<span style="color: var(--app-danger-color)">Error: ${err.message || err}</span>`;
  }
}

/**
 * Lock Vault Immediately
 */
async function lockVaultNow() {
  const frame = document.getElementById("guest-app-frame");
  if (frame) frame.src = "about:blank";

  await window.PixieVaultNative.lockVault();
  hostState.isLocked = true;
  showLockScreen();
}

/**
 * Show Lock Screen
 */
function showLockScreen() {
  document.getElementById("view-lock-screen").style.display = "flex";
  document.getElementById("app-environment").style.display = "none";
}

/**
 * Unlock and transition into the active workspace
 */
async function unlockAndShowWorkspace(workspaceId) {
  document.getElementById("view-lock-screen").style.display = "none";
  document.getElementById("app-environment").style.display = "flex";

  await refreshInstalledAppsList();
  switchWorkspace(workspaceId || "dashboard");
}

/**
 * Switch Active Workspace & Orchestrate 100% Native Source App
 */
async function switchWorkspace(workspaceId) {
  if (hostState.isLocked) {
    await unlockAndShowWorkspace(workspaceId);
    return;
  }

  hostState.activeWorkspace = workspaceId;

  // Sync Dropdown
  const select = document.getElementById("workspace-select");
  if (select) select.value = workspaceId;

  // View elements
  const wsDashboard = document.getElementById("ws-dashboard");
  const wsGuest = document.getElementById("ws-guest-container");
  const guestFrame = document.getElementById("guest-app-frame");
  const loadingOverlay = document.getElementById("guest-loading-overlay");
  const errorOverlay = document.getElementById("guest-error-overlay");
  const loadingTitle = document.getElementById("guest-loading-title");
  const loadingDesc = document.getElementById("guest-loading-desc");
  const errorMsg = document.getElementById("guest-error-msg");

  if (errorOverlay) errorOverlay.style.display = "none";

  if (workspaceId === "dashboard") {
    // Show Dashboard
    if (wsGuest) wsGuest.classList.remove("active");
    if (wsDashboard) wsDashboard.classList.add("active");
    if (guestFrame) guestFrame.src = "about:blank";

    await renderAppCards();
    await refreshBusTable();
    window.PixieVaultNative.unloadApp().catch(() => {});
    return;
  }

  // Find app metadata
  let appInfo = hostState.installedApps.find(a => a.manifest.app_id === workspaceId);
  if (!appInfo) {
    await refreshInstalledAppsList();
    appInfo = hostState.installedApps.find(a => a.manifest.app_id === workspaceId);
  }
  const appName = appInfo?.manifest?.name || workspaceId;

  // Switch to Universal Guest Container
  if (wsDashboard) wsDashboard.classList.remove("active");
  if (wsGuest) wsGuest.classList.add("active");

  // Show Loading Overlay
  if (loadingOverlay) {
    loadingOverlay.style.display = "flex";
    if (loadingTitle) loadingTitle.innerText = `Launching ${appName}...`;
    if (loadingDesc) loadingDesc.innerText = "Initializing application runtime and connecting interfaces...";
  }

  let targetUrl = "";
  try {
    const hasComposer = appInfo && (appInfo.is_composer || (appInfo.manifest?.composer && Object.keys(appInfo.manifest.composer.services || {}).length > 0));

    if (hasComposer) {
      if (loadingDesc) loadingDesc.innerText = "Starting native Composer services & probing loopback healthcheck...";
      const composerStatus = await window.PixieVaultNative.startComposerApp(workspaceId);
      if (composerStatus.error) {
        throw new Error(composerStatus.error);
      }
      targetUrl = composerStatus.entrypoint_url;
    } else {
      if (loadingDesc) loadingDesc.innerText = "Preparing offline guest viewport...";
      const composerStatus = await window.PixieVaultNative.startComposerApp(workspaceId);
      if (composerStatus && composerStatus.entrypoint_url) {
        targetUrl = composerStatus.entrypoint_url;
      } else if (appInfo && appInfo.launch_url) {
        targetUrl = appInfo.launch_url;
      } else {
        targetUrl = "index.html";
      }
    }

    // Set frame source to 100% native URL
    console.log(`[Host Shell Router] Navigating guest frame to: ${targetUrl}`);
    if (guestFrame) {
      guestFrame.src = targetUrl;
    }

    // Record active app in host registry
    window.PixieVaultNative.launchApp(workspaceId).catch(() => {});

  } catch (err) {
    console.error(`Failed to launch app ${workspaceId}:`, err);
    if (loadingOverlay) loadingOverlay.style.display = "none";
    if (errorOverlay) {
      errorOverlay.style.display = "flex";
      if (errorMsg) errorMsg.innerText = err.message || String(err);
      const repairBtn = document.getElementById("guest-repair-btn");
      if (repairBtn) {
        const isDepError = err.message && (
          err.message.includes("provision") ||
          err.message.includes("requirements") ||
          err.message.includes("dependency") ||
          err.message.includes("Dependency") ||
          err.message.includes(".venv") ||
          err.message.includes("runtime")
        );
        repairBtn.innerText = "Prepare Application Runtime";
        repairBtn.style.display = isDepError ? "inline-block" : "none";
      }
    }
  }
}

/**
 * Retry launching active workspace
 */
function retryActiveWorkspace() {
  switchWorkspace(hostState.activeWorkspace);
}

/**
 * Explicitly provision or repair runtime environment for active app
 */
async function repairActiveEnvironment() {
  const appId = hostState.activeWorkspace;
  if (!appId || appId === "dashboard") return;

  const loadingOverlay = document.getElementById("guest-loading-overlay");
  const loadingTitle = document.getElementById("guest-loading-title");
  const loadingDesc = document.getElementById("guest-loading-desc");
  const errorOverlay = document.getElementById("guest-error-overlay");

  if (errorOverlay) errorOverlay.style.display = "none";
  if (loadingOverlay) {
    loadingOverlay.style.display = "flex";
    if (loadingTitle) loadingTitle.innerText = "Preparing Application Runtime...";
    if (loadingDesc) loadingDesc.innerText = "Resolving packages and preparing declared service runtime environments...";
  }

  try {
    console.log(`[Host Shell] Triggering environment repair for ${appId}...`);
    const res = await window.PixieVaultNative.repairAppEnvironment(appId);
    console.log("[Host Shell] Provisioning success:", res);
    await switchWorkspace(appId);
  } catch (err) {
    console.error("[Host Shell] Provisioning failed:", err);
    if (loadingOverlay) loadingOverlay.style.display = "none";
    if (errorOverlay) {
      errorOverlay.style.display = "flex";
      const errorMsg = document.getElementById("guest-error-msg");
      if (errorMsg) errorMsg.innerText = `Environment Provisioning Failed:\n${err.message || String(err)}`;
    }
  }
}

/**
 * Render App Cards in Dashboard with Clean SVG Badges
 */
async function renderAppCards() {
  const grid = document.getElementById("dashboard-apps-grid");
  if (!grid) return;

  try {
    const apps = await window.PixieVaultNative.listInstalledApps();
    hostState.installedApps = Array.isArray(apps) ? apps : [];
    let html = "";

    for (const app of hostState.installedApps) {
      const manifest = app.manifest;
      const appId = manifest.app_id;
      
      let sourceBadge = `<span class="badge badge-accent">Local Folder</span>`;
      if (app.source) {
        if (app.source.type === "PortablePackage") {
          sourceBadge = `<span class="badge badge-success">Air-Gapped Package</span>`;
        } else if (app.source.type === "GitHubRelease") {
          const repo = app.source.details?.repository || "GitHub";
          const tag = app.source.details?.tag || "latest";
          sourceBadge = `<span class="badge badge-info">${repo}@${tag}</span>`;
        }
      }

      let iconContent = `<svg viewBox="0 0 24 24" width="24" height="24" stroke="var(--app-accent-color)" stroke-width="2" fill="none"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>`;
      let accentStyle = "";

      if (manifest.presentation) {
        if (manifest.presentation.accent) {
          accentStyle = `style="border-color: ${manifest.presentation.accent}33;"`;
        }
        if (manifest.presentation.icon) {
          const rawIcon = manifest.presentation.icon.trim();
          if (rawIcon.startsWith("<svg") || rawIcon.startsWith("&lt;svg")) {
            iconContent = rawIcon;
          } else if (rawIcon.endsWith(".svg") || rawIcon.endsWith(".png")) {
            iconContent = `<img src="${rawIcon}" width="24" height="24" alt="${manifest.name}">`;
          } else {
            iconContent = `<span style="font-size: 1.4rem; line-height: 1;">${rawIcon}</span>`;
          }
        }
      }

      html += `
        <div class="app-card" ${accentStyle} onclick="switchWorkspace('${appId}')">
          <div class="app-card-header">
            <div class="app-icon">${iconContent}</div>
            <div class="app-info">
              <h4 class="app-name">${manifest.name}</h4>
              <span class="app-version">v${manifest.version} • ${appId}</span>
            </div>
          </div>
          <p class="app-desc">${manifest.description || "100% native source application interface."}</p>
          <div class="app-card-footer">
            <button class="btn btn-primary btn-sm">Launch Native App</button>
            ${sourceBadge}
          </div>
        </div>
      `;
    }

    grid.innerHTML = html;
  } catch (err) {
    console.error("Failed to render app cards:", err);
  }
}

/**
 * Save Active Workspace State to Encrypted Vault
 */
async function saveActiveAppState() {
  const current = hostState.activeWorkspace;
  if (current === "dashboard") {
    alert("Currently viewing Ecosystem Dashboard. Select an application workspace to save.");
    return;
  }

  // Trigger postMessage to guest iframe to notify it to save
  const frame = document.getElementById("guest-app-frame");
  if (frame && frame.contentWindow) {
    frame.contentWindow.postMessage({ type: "PV_SAVE_REQUEST" }, "*");
  }

  try {
    await window.PixieVaultNative.saveAppData({ lastSaved: new Date().toISOString() }, current);
    alert(`✓ Vault state synchronized for ${current}.`);
  } catch (err) {
    alert(`Failed to save vault data: ${err.message || err}`);
  }
}

/**
 * Modal & Distribution Handlers
 */
function openInstallAppModal() {
  document.getElementById("modal-install-app").style.display = "flex";
}

function closeInstallAppModal() {
  document.getElementById("modal-install-app").style.display = "none";
  document.getElementById("install-status-msg").innerText = "";
}

async function triggerNativePackagePicker() {
  const statusEl = document.getElementById("install-status-msg");
  if (statusEl) statusEl.innerText = "Opening native OS package picker...";

  try {
    const res = await window.PixieVaultNative.pickAndInstallPackageFile();
    if (res) {
      if (statusEl) statusEl.innerHTML = `<span style="color:var(--app-success-color)">✓ Imported & Mounted ${res.manifest.name} successfully!</span>`;
      setTimeout(async () => {
        closeInstallAppModal();
        await refreshInstalledAppsList();
        switchWorkspace(res.manifest.app_id);
      }, 1000);
    }
  } catch (err) {
    if (statusEl) statusEl.innerHTML = `<span style="color:var(--app-danger-color)">${err.message || err}</span>`;
  }
}

async function triggerNativeFolderPicker() {
  const statusEl = document.getElementById("install-status-msg");
  if (statusEl) statusEl.innerText = "Opening native OS folder picker...";

  try {
    const res = await window.PixieVaultNative.pickAndInstallLocalFolder();
    if (res) {
      if (statusEl) statusEl.innerHTML = `<span style="color:var(--app-success-color)">✓ Mounted ${res.manifest.name} successfully!</span>`;
      setTimeout(async () => {
        closeInstallAppModal();
        await refreshInstalledAppsList();
        switchWorkspace(res.manifest.app_id);
      }, 1000);
    }
  } catch (err) {
    if (statusEl) statusEl.innerHTML = `<span style="color:var(--app-danger-color)">${err.message || err}</span>`;
  }
}

async function handleGitHubInstall(e) {
  if (e) e.preventDefault();
  const repo = document.getElementById("gh-repo-input").value.trim();
  const tag = document.getElementById("gh-tag-input").value.trim() || null;
  const statusEl = document.getElementById("install-status-msg");

  if (!repo) return;
  if (statusEl) statusEl.innerText = `Fetching release target ${repo}...`;

  try {
    const res = await window.PixieVaultNative.installGitHubApp(repo, tag);
    if (statusEl) statusEl.innerHTML = `<span style="color:var(--app-success-color)">✓ Registered ${repo} target!</span>`;
    setTimeout(async () => {
      closeInstallAppModal();
      await refreshInstalledAppsList();
      switchWorkspace(res.manifest.app_id);
    }, 1000);
  } catch (err) {
    if (statusEl) statusEl.innerHTML = `<span style="color:var(--app-danger-color)">Error: ${err.message || err}</span>`;
  }
}

async function promptExportCurrentPackage() {
  let appId = hostState.activeWorkspace;
  if (appId === "dashboard") {
    const installed = hostState.installedApps || [];
    if (installed.length === 0) {
      alert("No installed applications available to export.");
      return;
    }
    const appChoices = installed.map(a => `${a.manifest.app_id} (${a.manifest.name})`).join("\n• ");
    const chosen = prompt(`Select an application ID to export as a portable .pvpkg bundle:\n\n• ${appChoices}`, installed[0].manifest.app_id);
    if (!chosen) return;
    const matched = installed.find(a => a.manifest.app_id === chosen.trim());
    if (!matched) {
      alert(`Application ID '${chosen}' was not found in the installed applications registry.`);
      return;
    }
    appId = matched.manifest.app_id;
  }

  const defaultFilename = `${appId}_bundle.pvpkg`;
  const dest = prompt(`Enter destination file path for all-in-one .pvpkg portable bundle of '${appId}':`, defaultFilename);
  if (!dest) return;

  try {
    await window.PixieVaultNative.exportAppPackage(appId, dest, true);
    alert(`✓ Successfully exported all-in-one package: ${dest}\nIncludes application code and AES-256-GCM encrypted vault data.`);
  } catch (err) {
    alert(`Failed to export package: ${err.message || err}`);
  }
}

async function checkForAllUpdates() {
  try {
    const res = await window.PixieVaultNative.checkAppUpdates(hostState.activeWorkspace);
    alert(`Update Check:\n${res.release_notes || "All apps are up to date."}`);
  } catch (err) {
    alert(`Update check: ${err.message || err}`);
  }
}

/**
 * Register Public Exports to Host Inter-App Bus (Host-level telemetry only)
 */
function registerAllAppExports() {
  window.PixieVaultNative.registerDataExporter(() => ({
    hostVersion: "0.2.0",
    installedAppsCount: hostState.installedApps ? hostState.installedApps.length : 0,
    activeWorkspace: hostState.activeWorkspace
  }));
}

async function refreshBusTable() {
  const tbody = document.getElementById("bus-table-body");
  if (!tbody) return;

  try {
    const busData = await window.PixieVaultNative.getBusMetrics();
    let rows = [];

    for (const [appId, metrics] of Object.entries(busData || {})) {
      for (const [metricKey, metricVal] of Object.entries(metrics || {})) {
        const val = metricVal?.value !== undefined ? metricVal.value : metricVal;
        rows.push(`
          <tr>
            <td><code>${appId}</code></td>
            <td><strong>${metricKey}</strong></td>
            <td><span class="metric-highlight">${JSON.stringify(val)}</span></td>
            <td><span class="badge badge-success">Active Export</span></td>
          </tr>
        `);
      }
    }

    if (rows.length > 0) {
      tbody.innerHTML = rows.join("");
    }
  } catch (err) {
    console.error("Bus table refresh error:", err);
  }
}

/**
 * Theme Toggle Helpers
 */
function toggleThemeQuick() {
  currentThemeIdx = (currentThemeIdx + 1) % THEMES.length;
  const themeClass = THEMES[currentThemeIdx];
  document.body.className = themeClass;
}

function applyThemeByName(name) {
  if (name.includes("Emerald")) document.body.className = "theme-emerald";
  else if (name.includes("Sunset")) document.body.className = "theme-sunset";
  else if (name.includes("Solar")) document.body.className = "theme-solar";
  else document.body.className = "";
}

// Auto-run on DOM ready
document.addEventListener("DOMContentLoaded", initHostShell);
