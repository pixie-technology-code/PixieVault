/**
 * PixieVault Standard IPC Native Bridge (wrapper-bridge.js)
 * Robust invocation and event bridge between web application context and Tauri v2 Rust backend.
 * Features 100% app-agnostic runtime architecture, dynamically loaded demo catalog, and clean IPC delegation.
 */
(function () {
  const isTauriAvailable = !!(
    window.__TAURI__?.core?.invoke ||
    window.__TAURI_INTERNALS__?.invoke ||
    window.__TAURI__?.invoke
  );

  const isDemoMode = !!(
    window.__PIXIEVAULT_DEMO_MODE__ ||
    (typeof location !== "undefined" && location.search && location.search.includes("demo=1"))
  );

  // Helper to invoke Tauri IPC command across any Tauri v2 global binding
  async function invokeTauri(cmd, args = {}) {
    if (window.__TAURI__?.core?.invoke) {
      return await window.__TAURI__.core.invoke(cmd, args);
    }
    if (window.__TAURI_INTERNALS__?.invoke) {
      return await window.__TAURI_INTERNALS__.invoke(cmd, args);
    }
    if (window.__TAURI__?.invoke) {
      return await window.__TAURI__.invoke(cmd, args);
    }

    if (!isDemoMode) {
      const err = new Error(`[ERR_IPC_UNAVAILABLE] Native IPC call '${cmd}' failed: Tauri backend binding unavailable outside demo mode.`);
      err.code = "ERR_IPC_UNAVAILABLE";
      throw err;
    }

    return null;
  }

  // Dynamic Demo Catalog Resolver (for standalone browser demo mode and mock unit tests)
  function getDemoCatalog() {
    if (typeof window !== "undefined" && window.__PIXIEVAULT_DEMO_CATALOG__) {
      return window.__PIXIEVAULT_DEMO_CATALOG__;
    }
    if (typeof window !== "undefined" && window.__PIXIEVAULT_MOCK_ECOSYSTEM__) {
      return {
        apps: [],
        metrics: window.__PIXIEVAULT_MOCK_ECOSYSTEM__
      };
    }
    return {
      apps: [],
      metrics: {}
    };
  }

  let cachedVaultState = {
    isLocked: true,
    activeApp: null
  };

  window.PixieVaultNative = {
    isNative: isTauriAvailable,
    isDemoMode: isDemoMode,

    /**
     * Get overall Host Vault status (locked/unlocked, active app, biometrics)
     */
    async getVaultStatus() {
      const res = await invokeTauri("pv_get_vault_status");
      if (res !== null) return res;

      return {
        is_locked: cachedVaultState.isLocked,
        active_app: cachedVaultState.activeApp,
        biometrics_available: true,
        biometric_provider: "Demo Mock Provider",
        biometric_type: "Hardware Biometrics",
        auto_launch_last_app: false,
        last_opened_app: null
      };
    },

    /**
     * Get Windows Hello / Platform Protector capabilities for current device
     */
    async getWindowsHelloCapabilities() {
      const res = await invokeTauri("pv_windows_hello_capabilities");
      if (res !== null) return res;
      return {
        is_available: true,
        provider_name: "Mock Windows Hello Provider",
        biometric_type: "Windows Hello (Fingerprint / Face / PIN)",
        is_enrolled: false,
        availability_status: "Ready",
        supported_hardware: ["Fingerprint", "PIN", "TPM 2.0"],
        device_id: "mock-device-id",
        device_name: "Mock-PC"
      };
    },

    /**
     * Enroll Windows Hello / Platform Protector for this device
     */
    async enrollWindowsHello(passphrase = null) {
      const res = await invokeTauri("pv_windows_hello_enroll", { passphrase });
      if (res !== null) return res;
      return {
        id: "windows-hello-mock",
        protector_type: "windows-hello-cng",
        wrapped_master_key_b64: "bW9ja193cmFwcGVkX2tleQ=="
      };
    },

    /**
     * Unlock Vault using Windows Hello / Platform Protector
     */
    async unlockWindowsHello() {
      const res = await invokeTauri("pv_windows_hello_unlock");
      if (res !== null) {
        if (res.success) cachedVaultState.isLocked = false;
        return res;
      }
      cachedVaultState.isLocked = false;
      return {
        success: true,
        auto_launch_app: null,
        error: null
      };
    },

    /**
     * Revoke Windows Hello protector for this device
     */
    async revokeWindowsHello() {
      const res = await invokeTauri("pv_windows_hello_revoke");
      return res ?? true;
    },

    /**
     * Trigger OS Biometric Authentication (Windows Hello / Touch ID / PAM)
     */
    async authenticateBiometrics() {
      return this.unlockWindowsHello();
    },

    /**
     * Authenticate using Master Passphrase
     */
    async authenticatePassword(passphrase) {
      const res = await invokeTauri("pv_authenticate_password", { passphrase });
      if (res !== null) {
        if (res.success) cachedVaultState.isLocked = false;
        return res;
      }
      cachedVaultState.isLocked = false;
      return {
        success: true,
        auto_launch_app: null,
        error: null
      };
    },

    /**
     * Lock Vault immediately and flush memory keys
     */
    async lockVault() {
      const res = await invokeTauri("pv_lock_vault");
      cachedVaultState.isLocked = true;
      cachedVaultState.activeApp = null;
      if (typeof window !== "undefined") {
        delete window.__PIXIEVAULT_APP_DATA__;
        try {
          window.localStorage.clear();
          window.sessionStorage.clear();
        } catch (e) {}
      }
      return res ?? true;
    },

    /**
     * Setup or update Vault master passphrase
     */
    async setMasterPassphrase(passphrase, currentPassphrase = null) {
      const res = await invokeTauri("pv_set_master_passphrase", {
        passphrase,
        currentPassphrase
      });
      return res;
    },

    /**
     * Listen for automatic vault locking events
     */
    onAutoLock(callback) {
      this.onHostEvent("pv_vault_autolocked", () => {
        cachedVaultState.isLocked = true;
        cachedVaultState.activeApp = null;
        if (typeof window !== "undefined") {
          delete window.__PIXIEVAULT_APP_DATA__;
          try {
            window.localStorage.clear();
            window.sessionStorage.clear();
          } catch (e) {}
        }
        callback();
      });
    },

    /**
     * List all installed applications discovered in the host
     */
    async listInstalledApps() {
      const res = await invokeTauri("pv_list_apps");
      if (res !== null && Array.isArray(res)) return res;

      const catalog = getDemoCatalog();
      return catalog.apps || [];
    },

    /**
     * Launch or switch to an installed application
     */
    async launchApp(appId) {
      const res = await invokeTauri("pv_launch_app", { appId });
      cachedVaultState.activeApp = appId;
      return res || { success: true, appId };
    },

    /**
     * Unload active app and return to Host Shell Dashboard
     */
    async unloadApp() {
      const res = await invokeTauri("pv_unload_app");
      cachedVaultState.activeApp = null;
      return res ?? true;
    },

    /**
     * Load encrypted app state from vault
     */
    async loadAppData(appId) {
      const res = await invokeTauri("pv_load_app_data", { appId });
      if (res !== null && res !== undefined) return res;

      if (isDemoMode) {
        const raw = localStorage.getItem(`PV_VAULT_${appId || "default"}`);
        return raw ? JSON.parse(raw) : null;
      }
      return null;
    },

    /**
     * Persist updated app state into encrypted vault
     */
    async saveAppData(data, appId) {
      const res = await invokeTauri("pv_save_app_data", { appId, data });
      if (res !== null) return res;

      if (isDemoMode) {
        localStorage.setItem(`PV_VAULT_${appId || "default"}`, JSON.stringify(data));
        return true;
      }
      return false;
    },

    /**
     * Register a callback or metrics object for the Inter-App Bus
     */
    registerDataExporter(exportFnOrObj) {
      const getMetrics = () => typeof exportFnOrObj === "function" ? exportFnOrObj() : exportFnOrObj;
      window.__PIXIEVAULT_EXPORT_METRICS__ = getMetrics;

      const metrics = getMetrics();
      invokeTauri("pv_export_metrics", { metrics }).catch(err => {
        console.warn("[InterAppBus] Export failed:", err);
      });
    },

    /**
     * Query metrics exposed by an adjacent installed app
     */
    async requestCrossAppData(targetAppId, metricName, callerAppId) {
      const res = await invokeTauri("pv_query_adjacent_metric", {
        callerAppId,
        targetAppId,
        metricName
      });
      if (res !== null && res !== undefined) return res;

      const catalog = getDemoCatalog();
      const targetMetrics = catalog.metrics?.[targetAppId];
      if (targetMetrics && targetMetrics[metricName] !== undefined) {
        return targetMetrics[metricName];
      }
      throw new Error(`Metric '${metricName}' on app '${targetAppId}' not found.`);
    },

    /**
     * Get all exported metrics in the bus (for Host Inspector)
     */
    async getBusMetrics() {
      const res = await invokeTauri("pv_get_bus_metrics");
      if (res !== null) return res;

      const catalog = getDemoCatalog();
      return catalog.metrics || {};
    },

    // ================= Dual-Source Distribution APIs =================

    /**
     * Pick a .pvpkg package file with native OS file dialog
     */
    async pickAndInstallPackageFile() {
      return await invokeTauri("pv_pick_and_install_package_file");
    },

    /**
     * Pick a local directory containing manifest.json with native OS file dialog
     */
    async pickAndInstallLocalFolder() {
      return await invokeTauri("pv_pick_and_install_local_folder");
    },

    /**
     * Pick a local directory / package with native OS file dialog
     */
    async pickAndInstallLocalApp() {
      return await invokeTauri("pv_pick_and_install_local_app");
    },

    /**
     * Install an app from a local folder path
     */
    async installLocalDirectory(dirPath) {
      return await invokeTauri("pv_install_local_directory", { dirPath });
    },

    /**
     * Install an all-in-one .pvpkg package file
     */
    async installPackageFile(packageFile, targetDir = null) {
      return await invokeTauri("pv_install_package_file", { packageFile, targetDir });
    },

    /**
     * Export an application + optional encrypted vault database to a .pvpkg archive
     */
    async exportAppPackage(appId, outputFile, includeVaultData = true) {
      return await invokeTauri("pv_export_app_package", { appId, outputFile, includeVaultData });
    },

    /**
     * Install or register a GitHub release target
     */
    async installGitHubApp(repo, tag = null, publicKey = null) {
      return await invokeTauri("pv_install_github_app", { repo, tag, publicKey });
    },

    /**
     * Check for app updates
     */
    async checkAppUpdates(appId = "dashboard") {
      return await invokeTauri("pv_check_app_updates", { appId });
    },

    // ================= Python Sidecar Process APIs =================

    /**
     * Start a Python backend sidecar daemon
     */
    async startSidecar(appId, scriptPath, port = 5000) {
      const res = await invokeTauri("pv_start_sidecar", { appId, scriptPath, port });
      if (res !== null) return res;
      if (isDemoMode) {
        return {
          app_id: appId,
          is_running: true,
          port,
          url: `http://127.0.0.1:${port}`
        };
      }
      throw new Error("Sidecar unavailable");
    },

    /**
     * Stop a running sidecar process
     */
    async stopSidecar(appId) {
      const res = await invokeTauri("pv_stop_sidecar", { appId });
      return res ?? true;
    },

    /**
     * Get current status of a sidecar process
     */
    async getSidecarStatus(appId) {
      const res = await invokeTauri("pv_get_sidecar_status", { appId });
      if (res !== null) return res;
      if (isDemoMode) {
        return {
          app_id: appId,
          is_running: false,
          port: 5000,
          url: null
        };
      }
      return { app_id: appId, is_running: false, port: 0, url: null };
    },

    // ================= Native Vault Composer APIs =================

    /**
     * Start all native composer services defined in an app's manifest
     */
    async startComposerApp(appId) {
      const res = await invokeTauri("pv_composer_start_app", { appId });
      if (res !== null) return res;
      if (isDemoMode) {
        return {
          app_id: appId,
          is_running: true,
          services: {},
          entrypoint_url: "index.html",
          error: null
        };
      }
      throw new Error(`Failed to start Composer app '${appId}'`);
    },

    /**
     * Stop all native composer services for an app
     */
    async stopComposerApp(appId) {
      const res = await invokeTauri("pv_composer_stop_app", { appId });
      return res ?? true;
    },

    /**
     * Get runtime status of all composer services for an app
     */
    async getComposerStatus(appId) {
      const res = await invokeTauri("pv_composer_get_status", { appId });
      if (res !== null) return res;
      if (isDemoMode) {
        return {
          app_id: appId,
          is_running: false,
          services: {},
          entrypoint_url: "index.html",
          error: null
        };
      }
      return {
        app_id: appId,
        is_running: false,
        services: {},
        entrypoint_url: "index.html",
        error: null
      };
    },

    /**
     * Explicitly provision environment and dependencies
     */
    async provisionAppEnvironment(appId, force = false) {
      return await invokeTauri("pv_provision_app_environment", { appId, force });
    },

    /**
     * Force repair environment and reinstall dependencies
     */
    async repairAppEnvironment(appId) {
      return await invokeTauri("pv_repair_app_environment", { appId });
    },

    /**
     * Toggle OS Native Window Fullscreen
     */
    async toggleFullscreen() {
      const res = await invokeTauri("pv_toggle_fullscreen");
      if (res !== null) return res;
      if (!document.fullscreenElement) {
        await document.documentElement.requestFullscreen?.();
        return true;
      } else {
        await document.exitFullscreen?.();
        return false;
      }
    },

    /**
     * Listen for Host events emitted by Rust native menus or backend
     */
    onHostEvent(eventName, callback) {
      if (window.__TAURI__?.event?.listen) {
        window.__TAURI__.event.listen(eventName, (event) => callback(event.payload));
      }
      if (window.__TAURI_INTERNALS__?.listen) {
        window.__TAURI_INTERNALS__.listen(eventName, (event) => callback(event.payload));
      }
      window.addEventListener(eventName, (e) => callback(e.detail));
    }
  };

  window.VaultFlowNative = window.PixieVaultNative;
})();
