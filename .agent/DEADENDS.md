# Dead Ends

<!-- Register of approaches that didn't work. Prefix each entry with DEADEND. -->
<!-- Future agents: read this at session start to avoid wasting time. -->

## Format

- **DEADEND**: [What was tried]
  - **Why it failed**: [Root cause]
  - **Alternative**: [What works instead]

- **DEADEND**: Treating PixieVault as a web app with browser-mode dev fallbacks (`localStorage`, mock browser servers, web-embedded menus).
  - **Why it failed**: PixieVault is a 100% native desktop application environment. Web-based menus and browser storage violate zero-trust architecture and create confusing, inconsistent multi-app interfaces.
  - **Alternative**: Pure native Rust shell (Tauri v2) that owns the window, native menuing system (File/Security/Apps/Storage/View), OS biometric integration (Windows Hello / Touch ID), and AES-256-GCM encrypted vault container.

- **DEADEND**: Relying on unpopulated `bundle.resources` glob patterns (e.g., `apps/**/*`) in `tauri.conf.json` on clean CI runners when the directory is `.gitignore`d.
  - **Why it failed**: Tauri v2's `tauri-build` fails during build time on clean clones with `glob pattern apps/**/* path not found or didn't match any files` if 0 files match the glob.
  - **Alternative**: Ensure `build.rs` creates the directory and writes a placeholder `.gitkeep` file if empty before `tauri-build` runs, and ensure `src/lib.rs` safely creates `bundled_apps_dir` on runtime startup.

- **DEADEND**: Gating Linux system calls like `prctl(PR_SET_PDEATHSIG, ...)` behind `#[cfg(unix)]`.
  - **Why it failed**: macOS (Darwin) and BSD are Unix systems, but `prctl` is a Linux-specific kernel API. On macOS arm64/x86_64, this caused the linker to fail with `Undefined symbols: _prctl`.
  - **Alternative**: Always gate Linux-specific syscalls with `#[cfg(target_os = "linux")]`.

- **DEADEND**: Publishing IDE task trackers (`tasks/`) or review scratchpads (`codereview/`) to public repositories.
  - **Why it failed**: Internal task lists, scratchpads, and planning artifacts pollute public repository history and cause merge conflicts across collaborator branches.
  - **Alternative**: Keep `tasks/` and `codereview/` in `.gitignore` and untracked from git (`git rm -r --cached tasks/`) so local IDE task management remains private.

- **DEADEND**: Omitting `fail_on_unmatched_files: false` in `softprops/action-gh-release@v2` across multi-platform build matrices.
  - **Why it failed**: When multiple OS runners share a matrix release workflow, any runner looking for non-existent sibling assets (.dmg on Linux or .deb on Windows) aborts the release step if strict matching is enabled.
  - **Alternative**: Set `fail_on_unmatched_files: false` in the release action and list glob patterns for all matrix targets (`*.AppImage`, `*.deb`, `*.exe`, `*.dmg`).

