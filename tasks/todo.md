# PixieVault Tasks & Progress Checklist

## 12. Clean Legacy Demo Apps & Migrate Cairn: Dead Reckoning
- [x] Phase 1: Clean legacy demo apps (`powertrain_analyzer`, `track_telemetry`, `wealthflow`) and remove root powertrain files <!-- id: 1201 -->
- [x] Phase 2: Copy `Cairn: Dead Reckoning` into `apps/cairn_dead_reckoning` (original source strictly untouched) <!-- id: 1202 -->
- [x] Phase 3: Create PixieVault `manifest.json`, vector SVG icon, and integrate `wrapper-bridge.js` / storage hooks <!-- id: 1203 -->
- [x] Phase 4: Stage package for Tauri (`src-tauri/apps/cairn_dead_reckoning`) and generate `.pvpkg` bundle in `dist/` <!-- id: 1204 -->
- [x] Phase 5: Update catalogs and test suites (`host/demo-catalog.json`, `tests/run-all-tests.js`, `integration_tests.rs`) <!-- id: 1205 -->
- [x] Phase 6: Run full verification (Cairn 23-suite tests + PixieVault 4-tier `./test-all.sh`) <!-- id: 1206 -->

## 13. Public Repository Preparation & GitHub Push
- [ ] Phase 1: Update `.gitignore` to strictly exclude all guest apps (`apps/`, `MikrotikFleetMgr/`, `src-tauri/apps/`, `dist/`), temporary databases, and caches <!-- id: 1301 -->
- [ ] Phase 2: Create high-impact, professional `README.md` showcasing architecture, features, security model, and quickstart <!-- id: 1302 -->
- [ ] Phase 3: Stage all core wrapper files and verify `git status` contains zero app packages or leaked artifacts <!-- id: 1303 -->
- [ ] Phase 4: Configure remote `https://github.com/pixie-technology-code/PixieVault` and push <!-- id: 1304 -->

