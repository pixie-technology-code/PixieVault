# PixieVault Application Migration Prompt for Coding Agents

Copy the prompt below into a coding agent and replace the values in angle brackets. The agent must perform a faithful migration and verify it through the native host—not merely generate a manifest.

````markdown
You are responsible for migrating an existing application into the PixieVault native desktop package format.

## Inputs

- Source application: `<SOURCE_DIRECTORY>`
- PixieVault repository: `<PIXIEVAULT_REPOSITORY>`
- Package ID: `<APP_ID>`
- Display name: `<APP_NAME>`
- Targets: Windows, Linux, and macOS unless explicitly excluded

## Objective

Create a faithful, self-contained PixieVault guest application at `apps/<APP_ID>/` and produce a `.pvpkg` bundle. Preserve the real workflows, data model, validation, and behavior. PixieVault is the entire native interface; never open or depend on an external browser.

Do not replace the application with a mock, screenshot, partial rewrite, or static demo. Do not delete or destructively edit its source. Work in the PixieVault package copy or an adapter layer. If faithful migration requires changing upstream source, report the reason before doing so.

## Working method

1. Read PixieVault's build/packaging documentation, relevant repository instructions, existing manifests, bridge API, and validators before editing.
2. Inventory the source: framework, build, entrypoints, routes, backends, assets, fonts, icons, workers, dynamic imports, persistence, databases, caches, secrets, logs, environment variables, ports, subprocesses, authentication, redirects, startup jobs, and network access.
3. Select and state a migration pattern: static frontend, frontend with bridge adapter, Composer-managed sidecar, or justified hybrid.
4. Write a short plan and risk list, then implement it. Keep an audit mapping every original feature to its migrated implementation and test.

## Package contract

Create `apps/<APP_ID>/manifest.json` with valid metadata, semantic versioning, a required `min_pixievault_version`, least-privilege permissions, theme compatibility, and a real entrypoint. Static apps use a package-relative entrypoint such as `index.html`. Composer apps use `http://127.0.0.1:{{services.backend.port}}/` and declare their runtime, working directory, dependency fingerprints, environment, automatic port, sandbox, and health check.

### PixieVault compatibility policy

`min_pixievault_version` is a compatibility floor, not the PixieVault version used during migration. Set it to the **oldest released PixieVault version that supplies every host capability the app actually needs**. Never copy the current/latest host version by default, use a future or unreleased version, or raise the floor for convenience, cosmetic differences, newer tooling, or features the app does not use.

Use this capability history as the authoritative selection guide. Preserve and extend it whenever PixieVault gains an app-facing capability; never rewrite old entries to describe newer behavior.

| PixieVault version | First app-facing capabilities available |
| --- | --- |
| `0.1.0` | Static packaged apps; package-relative entrypoints; manifest identity and app semantic version; requested read/write permissions; light/dark theme metadata; native bridge persistence and inter-app telemetry; `.pvpkg` packaging. |
| `0.2.0` | Composer-managed sidecars; templated host-assigned loopback ports; declarative health checks and sandbox settings; managed Python/Node runtime provisioning and dependency fingerprints; per-app mutable-data environment paths. |

#### Maintaining this history when PixieVault changes

Updating this migration prompt is a required part of every PixieVault release that changes the host version or any app-facing behavior. A PixieVault release is not migration-documentation complete until the maintainer has reviewed this section, even when the conclusion is that no new capability entry is needed.

For each release:

1. Read the new canonical PixieVault version from the native host configuration and confirm that all version-bearing project files agree.
2. Audit the release diff for new or changed manifest fields, bridge APIs, permissions, packaging rules, Composer/runtime behavior, storage and environment contracts, security restrictions, WebView assumptions, and installer behavior.
3. If the release introduces an app-facing capability that a migrated app may depend on, append a new row for that exact released SemVer and list only capabilities first available in that release. Do not duplicate unchanged capabilities from earlier rows.
4. If the version changes but adds no app-facing capability, do not add an empty or misleading capability row. Record in the release review that the table was checked and remains unchanged.
5. Never edit, collapse, reorder, or reinterpret a historical row after its release. If an old entry is materially wrong, add a clearly labeled correction note that preserves the original claim and explains the evidence; do not silently rewrite history.
6. Update the manifest example, validators, packaging templates, application-building documentation, and installer compatibility tests whenever the manifest compatibility contract itself changes.
7. Re-evaluate bundled apps against the expanded history. Raise an app's `min_pixievault_version` only when that app begins using a capability first provided by a newer release. Never bulk-replace existing minimums with the new host version.
8. Add or update tests proving that an app at or below the boundary is handled correctly: compatible hosts proceed, older hosts stop before mutation and offer update/cancel, cancellation is non-destructive, and a successful host update causes the manifest to be re-read before installation resumes.

The release pull request or completion report must state: the released PixieVault version, whether app-facing capabilities changed, the capability-history rows added or intentionally left unchanged, affected bundled-app minimums and their rationale, documentation/tooling/tests updated, and any compatibility boundary not actually exercised.

Determine the floor capability-by-capability:

1. Inventory every PixieVault-owned API, manifest key, bridge method, Composer feature, runtime facility, permission, and storage/environment contract used by the migrated app.
2. Map each required capability to the version where it first appeared in the table. The highest of those versions is the app's minimum.
3. Prefer compatibility adaptations when they preserve behavior without weakening security. Do not remove required functionality merely to claim an older floor.
4. Record the mapping and rationale in the completion report. If a required capability is missing from the history, do not guess: mark the migration blocked until the history is updated with an evidence-backed release version.
5. Test on the declared minimum when that build is available, as well as the current host. If the minimum cannot be exercised, say `not tested` and do not claim verified compatibility.

Installer contract: PixieVault compares its own SemVer with `min_pixievault_version` before copying, extracting, provisioning, launching, or replacing app data. If the host is too old, installation must pause and show the app name/version, installed PixieVault version, required minimum, and the capabilities responsible for that floor. Offer an explicit **Update PixieVault** action plus **Cancel**; never silently update, install anyway, or partially install. After a successful trusted update, re-read and re-validate the manifest before resuming. Offline/unavailable updates must leave the existing app and data untouched and provide a clear manual-update path. A newer host is compatible unless a future manifest contract explicitly introduces a separately named maximum-version constraint.

For a sidecar, follow this shape but adapt it to the real runtime; do not blindly copy commands or permissions:

```json
{
  "app_id": "<APP_ID>",
  "name": "<APP_NAME>",
  "version": "1.0.0",
  "min_pixievault_version": "<LOWEST_REQUIRED_VERSION>",
  "description": "<ACCURATE_DESCRIPTION>",
  "entrypoint": "http://127.0.0.1:{{services.backend.port}}/",
  "presentation": {
    "icon": "assets/app-icon.svg",
    "accent": "#6366f1",
    "category": "<CATEGORY>"
  },
  "composer": {
    "version": "1",
    "services": {
      "backend": {
        "command": ["python3", "app.py"],
        "working_dir": "backend",
        "runtime": {
          "type": "python",
          "requirements": "requirements.txt",
          "fingerprint_files": ["requirements.txt"]
        },
        "port": "auto",
        "environment": {
          "PORT": "{{port}}",
          "HOST": "127.0.0.1"
        },
        "healthcheck": {
          "endpoint": "/healthz",
          "interval_ms": 250,
          "timeout_ms": 30000,
          "expected_status": 200
        },
        "sandbox": {
          "enabled": true,
          "writable_dirs": ["."],
          "isolate_network_loopback": true
        }
      }
    }
  },
  "permissions": {
    "requested_read": [],
    "requested_write": []
  },
  "theme_compatibility": {
    "supports_dark_mode": true,
    "supports_light_mode": true
  }
}
```

Every referenced HTML, CSS, JavaScript, manifest, font, image, and runtime file must exist inside the package or be explicitly supplied by the native host. Paths cannot escape the configured web root. Do not reference `../shared/...` unless validation proves it is inside that root; package the approved bridge asset when necessary.

## Rendering and assets

Assume WebView2 on Windows, WKWebView on macOS, and WebKitGTK on Linux—not the developer's normal browser.

1. Do not use Unicode emoji, dingbats, private-use characters, or icon-font codepoints as functional UI icons. This includes navigation, status, buttons, theme controls, tables, dialogs, search, empty states, and manifest icons.
2. Replace functional glyphs with packaged or inline SVG. Use a consistent visual system, `currentColor` for theme adaptation, explicit dimensions, accessible names, and text labels where meaning is ambiguous.
3. Search HTML, templates, CSS `content`, JavaScript-generated markup, backend-generated strings, and manifest fields for Unicode symbols and icon-font classes. Classify every match as user content, ordinary typography, or an asset requiring replacement.
4. Do not depend on system emoji fonts, system icon fonts, CDNs, Google Fonts, or internet access. Package appropriately licensed fonts/assets locally. Declare required weights with `@font-face`.
5. Use PixieVault theme variables and verify contrast, focus, disabled, hover, selected, and status states in light and dark modes.
6. Test desktop-webview behavior: scrolling, sticky elements, dialogs, menus, tables, native form controls, long text, viewport reduction, and DPI/zoom scaling.

## Storage and filesystem

Installed application resources are immutable. Never write databases, keys, settings, caches, logs, uploads, generated files, or temporary state beside source files or beneath the resource directory.

Use the approved `window.PixieVaultNative` APIs for frontend persistence. Treat missing IPC as an explicit development/test condition; do not silently fall back to unencrypted `localStorage` in production.

For sidecars, route every mutable path through host-provided application-data locations such as `VAULT_STORAGE_DIR`, `VAULT_APP_DATA`, and `APP_DB_PATH`. Resolve and create those paths before database initialization.

- Never package `.venv`, `node_modules`, databases, secrets, caches, or platform-generated binaries unless the package specification explicitly requires them.
- Keep secrets out of resources and logs.
- Never put SQLite data on UNC, SMB, `\\wsl.localhost`, or mapped network storage. Use native local application data.
- Use SQLite WAL only on a verified local filesystem. Detect network paths and use a compatible journal mode or fail clearly.
- Normalize Windows child-process paths, remove incompatible `\\?\` prefixes while preserving UNC semantics, use absolute script paths, and never assume the repository is the working directory.

## Composer sidecars

1. Bind only to `127.0.0.1` unless broader access is explicitly required.
2. Read the host-assigned port; never hard-code one.
3. Provide an unauthenticated `/healthz` that returns 200 quickly, never redirects, performs no external I/O, and reports ready only after required local initialization succeeds.
4. Establish the listening service before optional schedulers, polling, discovery, backups, and expensive jobs, or start those asynchronously with bounded failure handling.
5. Emit flushed startup stages and fatal errors. Never swallow initialization exceptions or prompt interactively.
6. Ensure stdout/stderr are drainable and shutdown terminates subprocesses and workers.
7. Resolve runtimes cross-platform. On Windows validate Python candidates and avoid the Microsoft Store alias stub; use PixieVault provisioning and dependency fingerprints.
8. **Self-Contained Dependencies**: Audit all `try/import` blocks in upstream code that fall back to shell CLI subprocesses (e.g. attempting `import paramiko` with a fallback to `sshpass`/`scp`/`ssh` CLI). In desktop sandboxes and on Windows, host CLI utilities do not exist. Declare all network transport, protocol, and crypto libraries explicitly in `requirements.txt` or `package.json` so the provisioned virtualenv is 100% self-contained.
9. **Standard I/O Codecs & Bytecode**: When spawning Python subprocesses with piped I/O on Windows, Python defaults to the active OEM code page (`cp1252`), which crashes on non-ASCII prints. Ensure the host environment injects `PYTHONIOENCODING=utf-8`, `PYTHONUTF8=1`, and `PYTHONDONTWRITEBYTECODE=1`, and sanitize backend diagnostic prints with ASCII log prefixes (`[backend]`, `[OK]`, `[ERROR]`, `[WARN]`).
10. Treat health-check timeout as a symptom requiring process state and captured logs, never as the final diagnosis.


## Security and navigation

- Keep navigation inside PixieVault; no external-browser fallback.
- Broker filesystem, shell, download, clipboard, URL-scheme, and external-navigation operations through approved native APIs.
- Keep authentication independent from `/healthz`.
- Apply least privilege to manifest, filesystem, and network access.
- Do not weaken CSP or add `unsafe-*` without a documented, tested requirement.

## Required verification

Do not claim completion from a build or normal-browser test alone. Run or add tests proving:

1. the manifest validates, `min_pixievault_version` is valid SemVer and justified by the capability history, and every asset resolves from the packaged web root;
2. `dist/<APP_ID>.pvpkg` builds successfully;
3. the app launches through the real PixieVault host/Composer path on an ephemeral port;
4. `/healthz` and the entrypoint respond correctly;
5. early backend exits surface captured logs rather than timing out;
6. mutable state is written only to assigned application data and survives restart;
7. no secret, database, cache, or virtual environment enters the package;
8. principal workflows match the source application;
9. no functional emoji or icon-font glyph remains;
10. there are no tofu boxes, missing assets, console errors, or blocked CSP requests;
11. light/dark themes and common DPI/zoom values work;
12. shutdown leaves no orphan sidecar;
13. Windows works from a path containing spaces and, where relevant, a WSL/UNC source checkout while runtime data remains on local Windows storage;
14. Linux WebKitGTK works without an installed color-emoji font.

Use existing validators and test runners first. Add focused regressions for discovered failures. Never weaken tests to conceal a defect.

## Completion report

Return the selected migration pattern and rationale, changed files, feature-parity audit, Unicode/asset audit, storage/environment contract, sidecar command/bind/port/health contract, compatibility-floor capability mapping, tests on the declared minimum and current PixieVault version, package path, and unresolved risks. Distinguish facts from assumptions. Mark any platform or host version not actually exercised as `not tested`; never infer portability from another platform or version.
````

For backend migrations, give the agent access to a clean or representative environment for each supported OS. Direct browser testing is not evidence that a PixieVault migration works.
