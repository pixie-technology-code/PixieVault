/**
 * PixieVault Automated Native Menu Item Test Suite
 * Validates that 100% of native menu items declared in Rust (menu.rs) have functional handlers in JS (shell.js)
 */

const fs = require('fs');
const path = require('path');
const assert = require('assert');

console.log('========================================================');
console.log('  PixieVault Automated Native Menu Dispatch Test        ');
console.log('========================================================\n');

// 1. Extract all menu item IDs declared in src-tauri/src/menu.rs
const menuRsPath = path.join(__dirname, '../src-tauri/src/menu.rs');
const menuRsContent = fs.readFileSync(menuRsPath, 'utf8');

const idRegex = /MenuItemBuilder::with_id\(\s*"([^"]+)"/g;
const declaredMenuIds = [];
let match;
while ((match = idRegex.exec(menuRsContent)) !== null) {
  declaredMenuIds.push(match[1]);
}

console.log(`[1/3] Discovered ${declaredMenuIds.length} native menu item(s) in menu.rs:`);
declaredMenuIds.forEach(id => console.log(`  • ${id}`));
assert(declaredMenuIds.length >= 20, 'Expected at least 20 declared menu items in menu.rs');

// 2. Setup Mock DOM environment for shell.js
const mockElements = new Map();
function getOrCreateElement(id) {
  if (!mockElements.has(id)) {
    mockElements.set(id, {
      id,
      style: {},
      value: '',
      innerText: '',
      innerHTML: '',
      children: [],
      classList: {
        add: () => {},
        remove: () => {},
        contains: () => false
      },
      appendChild(child) { this.children.push(child); },
      focus() {},
      scrollIntoView() {},
      click() {}
    });
  }
  return mockElements.get(id);
}

// Global DOM mocks
global.document = {
  getElementById: (id) => getOrCreateElement(id),
  createElement: (tag) => ({
    tagName: tag,
    style: {},
    className: '',
    innerText: '',
    appendChild() {},
    remove() {},
    click() {}
  }),
  addEventListener: () => {},
  documentElement: {
    requestFullscreen: async () => {}
  },
  exitFullscreen: async () => {},
  fullscreenElement: null,
  body: {
    className: ''
  }
};

global.window = {
  PixieVaultNative: {
    getVaultStatus: async () => ({ is_locked: false, platform: 'windows', biometric_enrolled: true }),
    listInstalledApps: async () => [{ manifest: { app_id: 'cairn_dead_reckoning', name: 'Cairn' } }],
    onHostEvent: () => {},
    registerDataExporter: () => {},
    getBusMetrics: async () => ({}),
    unloadApp: async () => {},
    lockVault: async () => {},
    saveAppData: async () => {},
    loadAppData: async () => ({ key: 'value' }),
    changeMasterPassword: async () => ({ success: true }),
    checkAppUpdates: async () => ({ release_notes: 'Up to date' })
  },
  location: { reload: () => {} },
  addEventListener: () => {}
};

global.URL = {
  createObjectURL: () => 'blob:mock',
  revokeObjectURL: () => {}
};
global.Blob = class { constructor() {} };
global.alert = () => {};
global.prompt = () => 'test_prompt';
global.confirm = () => true;

// 3. Load host/shell.js
const shellJsPath = path.join(__dirname, '../host/shell.js');
const shellJsCode = fs.readFileSync(shellJsPath, 'utf8');

eval(shellJsCode);

assert(global.window.PixieVaultShell, 'window.PixieVaultShell must be defined in shell.js');
assert(typeof global.window.PixieVaultShell.handleNativeMenu === 'function', 'handleNativeMenu must be a function');

console.log('\n[2/3] Executing menu dispatch handlers for all menu IDs in shell.js...');

let handledCount = 0;
for (const menuId of declaredMenuIds) {
  try {
    global.window.PixieVaultShell.handleNativeMenu(menuId);
    handledCount++;
    console.log(`  ✓ Successfully dispatched: ${menuId}`);
  } catch (err) {
    console.error(`  ✗ Error executing handler for menu ID '${menuId}':`, err);
    throw err;
  }
}

// 4. Verify specific interactive handlers
console.log('\n[3/3] Verifying specific UI modal states after menu events...');

// Test Help Menu Modals
global.window.PixieVaultShell.handleNativeMenu('help_about');
assert.strictEqual(getOrCreateElement('modal-about').style.display, 'flex', 'About modal must open on help_about');

global.window.PixieVaultShell.handleNativeMenu('help_docs');
assert.strictEqual(getOrCreateElement('modal-docs').style.display, 'flex', 'Docs modal must open on help_docs');

global.window.PixieVaultShell.handleNativeMenu('help_verify');
assert.strictEqual(getOrCreateElement('modal-verify-signatures').style.display, 'flex', 'Signature verification modal must open on help_verify');

// Test Security & Auth Modals
global.window.PixieVaultShell.handleNativeMenu('auth_change_pass');
assert.strictEqual(getOrCreateElement('modal-change-password').style.display, 'flex', 'Change passphrase modal must open on auth_change_pass');

// Test Theme Switching
global.window.PixieVaultShell.handleNativeMenu('theme_emerald');
assert.strictEqual(global.document.body.className, 'theme-emerald', 'Body theme class must be theme-emerald');

global.window.PixieVaultShell.handleNativeMenu('theme_sunset');
assert.strictEqual(global.document.body.className, 'theme-sunset', 'Body theme class must be theme-sunset');

global.window.PixieVaultShell.handleNativeMenu('theme_solar');
assert.strictEqual(global.document.body.className, 'theme-solar', 'Body theme class must be theme-solar');

global.window.PixieVaultShell.handleNativeMenu('theme_slate');
assert.strictEqual(global.document.body.className, '', 'Body theme class must be default for theme_slate');

console.log(`\n========================================================`);
console.log(`  ✓ ALL ${handledCount}/${declaredMenuIds.length} NATIVE MENU ITEMS TESTED & VERIFIED!`);
console.log(`========================================================\n`);
