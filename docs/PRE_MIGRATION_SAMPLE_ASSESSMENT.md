# Pre-Migration Baseline: Four Private-Data Applications

**Assessment date:** 2026-09-01  
**PixieVault baseline:** current workspace (revision must be pinned before a release decision)  
**Targets:** Windows, Linux, macOS  
**Mode:** faithful core workflows; offline after installation; optional cloud integrations may remain disabled

This is a jump-start assessment using `PRE_MIGRATION_PROMPT.md`. It is not a completed source audit: the upstream repositories were inspected through their current primary GitHub material, but exact commit SHAs were not checked out and native packages were not built or launched. Consequently, no application can receive `GO`. The findings are deliberately conservative.

## Portfolio verdict

| Application | Preliminary verdict | Closest migration shape | Dominant blockers |
| --- | --- | --- | --- |
| [Allos](https://github.com/FloorLamp/allos) | `NO_GO_HOST_GAP` | Node Composer sidecar, optional scheduler service | Hermetic Node/native dependency artifacts, Node 24 enforcement, encrypted file-tree lifecycle, verified cross-platform launch |
| [Actual Budget](https://github.com/actualbudget/actual) | `NO_GO_HOST_GAP` | Prebuilt frontend plus Node local service, or adapted local desktop architecture | Monorepo build artifacts, native SQLite dependency, runtime/version reproducibility, file import/export and download brokering, cross-WebView verification |
| [Baby Buddy](https://github.com/babybuddy/babybuddy) | `NO_GO_HOST_GAP` | Python/Django Composer sidecar with prebuilt static assets | Pinned Python/wheel provisioning, migration lifecycle, media paths, production server contract, cross-platform native dependencies |
| [Paperless-ngx](https://github.com/paperless-ngx/paperless-ngx) | `NO_GO_HOST_GAP` | Multi-service Python application with workers and managed native tools | Durable worker/broker topology, OCR/conversion binaries, consumption directory/watcher, large document trees, database/service orchestration |

`NO_GO_HOST_GAP` means “do not start a faithful migration on the current contract,” not “the application is unsuitable forever.” The recurring gaps below are the platform roadmap.

## Current PixieVault evidence and conflicts

The host has a useful base: static apps, a Composer manifest with multiple named services, automatic loopback ports, environment templates, health checks, Python/Node/custom/binary runtime categories, per-app data environment injection, sandbox configuration, and process teardown code/tests.

However, these issues prevent an unconditional compatibility decision:

1. **Compatibility floor is documented but not implemented in the manifest model.** `docs/MIGRATION_PROMPT.md` requires `min_pixievault_version`; `src-tauri/src/app_manager/manifest.rs` does not define or validate it. Serde ignores the unknown JSON field by default, so its presence does not enforce installation compatibility.
2. **A runtime type is not a hermetic runtime artifact.** Current provisioning invokes locally available Python/Node/package tooling and dependency installation. The four targets need pinned runtime versions, offline artifacts, native architecture selection, integrity metadata, and reproducible install behavior.
3. **Sandbox declarations need target-specific enforcement evidence.** Security-sensitive GO decisions require tests showing writable path, loopback, outbound network, subprocess, and child-process behavior on each target—not just parsing of the fields.
4. **The persistence claim needs a sidecar lifecycle proof.** Sidecar databases and uploads use mutable directories rather than the frontend state envelope. The gate must verify how those files are encrypted/materialized, flushed, unmounted, and recovered on lock/crash before describing them as protected at rest.
5. **Generic desktop capabilities are not represented in guest manifests.** The assessed apps need some combination of file import/export, downloads, drag/drop, embedded document viewing, notifications, and controlled egress. Telemetry read/write permissions do not express these authorities.

## Reusable gap register

| Gap | Generic capability needed | Why it matters | Applications blocked |
| --- | --- | --- | --- |
| `PV-GAP-001` | Enforced `min_pixievault_version` manifest and installer contract | Prevents installing against a host missing required security/lifecycle behavior | All four |
| `PV-GAP-002` | Hermetic, versioned runtime and dependency artifacts | Enables offline, reproducible Python/Node provisioning, native wheels/modules, integrity checks, and per-OS/architecture selection | Allos, Actual, Baby Buddy; part of Paperless |
| `PV-GAP-003` | Declarative native/system tool bundles | Provides licensed, checksummed executables/libraries without assuming Docker, PATH, a compiler, or administrator access | Paperless; potentially image/PDF features elsewhere |
| `PV-GAP-004` | First-class service roles and dependency graph | Expresses web, worker, scheduler, broker, one-shot migration, readiness, restart policy, and ordered shutdown generically | Allos scheduler, Baby Buddy migrations/jobs, Paperless |
| `PV-GAP-005` | Enforced outbound-network policy | Allows named hosts/protocols and explicit user consent while keeping offline/default-deny meaningful | Optional Allos integrations, Actual bank sync, Baby Buddy integrations, Paperless mail/webhooks |
| `PV-GAP-006` | Brokered desktop file ingress/egress | Covers picker/drop/import/export/download/print with scoped user intent and safe staging paths | All four |
| `PV-GAP-007` | Encrypted mutable file-tree lifecycle | Defines materialization, quotas, atomic backup/snapshot, crash recovery, lock flushing, deletion, and large uploads beyond key/value bridge state | All four, especially Paperless |
| `PV-GAP-008` | Cross-platform sandbox conformance suite | Proves loopback binding, writable paths, egress, subprocess containment, log draining, and descendant teardown on Windows/macOS/Linux | All four |
| `PV-GAP-009` | WebView application conformance suite | Tests cookies, service workers, WebSockets, downloads, blobs, PDF/media, CSP, routing, and popup behavior across all three WebViews | Actual, Baby Buddy, Paperless; Allos to a lesser degree |
| `PV-GAP-010` | Managed local infrastructure services | Declaratively provisions embedded databases, queues/brokers, search/index services, health, backup, and upgrades | Paperless |
| `PV-GAP-011` | Watched/staged ingestion directories | Safely consumes scanner/drop-folder input without granting arbitrary filesystem access or racing vault lock | Paperless |

These contracts should remain application-neutral. Trusted host code must never contain an `allos`, `actual`, `babybuddy`, or `paperless` branch.

## 1. Allos — private health vault

### Application evidence

Upstream describes a Next.js 16/React 19/Node 24 application using `better-sqlite3`. Persistent state includes `allos.db`, uploads, logs, and backups beneath `DATA_DIR`. Docker Compose launches the web app and a small scheduler service. AI, notifications, email, and third-party health integrations are optional; manual records and local analysis remain useful without them.

### Preliminary capability matrix

| Requirement | Current PixieVault state | Disposition |
| --- | --- | --- |
| Node 24 with `better-sqlite3` on all targets | `IMPLEMENTED_UNVERIFIED` runtime family; exact version/native artifact contract absent | `PV-GAP-002` |
| Web service on assigned loopback port | Implemented; relevant generic tests exist, app not exercised | Application adapter plus verification |
| Scheduler as second managed process | Multiple services parse/start, but scheduler roles/order/restart semantics are not explicit | `PV-GAP-004` |
| SQLite, uploads, logs, backups under one data root | Data env paths are documented; encrypted file-tree lifecycle not proven here | `PV-GAP-007` |
| Medical document import/export | No generic guest file authority in the manifest | `PV-GAP-006` |
| Default-deny optional external integrations | Sandbox field exists; enforceable per-destination egress is absent/unverified | `PV-GAP-005`, `PV-GAP-008` |

### Verdict

`NO_GO_HOST_GAP`. Allos remains the best first target after closing `PV-GAP-001`, `002`, `006`, `007`, and `008`. The scheduler can initially be an accepted exclusion only if reminders and automatic backups are explicitly excluded from required scope; otherwise close `PV-GAP-004` too.

Adapter effort after closure: **medium**. Reusable host effort: **large**.

## 2. Actual Budget — private finance vault

### Application evidence

Actual is a mature local-first Node/TypeScript monorepo with shared core, desktop UI, Electron packaging, local budget files, SQLite, optional sync, and import/export behavior. Its existing local-only desktop mode is strong architectural evidence that an offline core is possible, but it does not prove that a server build can simply be copied into PixieVault.

### Preliminary capability matrix

| Requirement | Current PixieVault state | Disposition |
| --- | --- | --- |
| Reproducible Yarn workspace build and packaged runtime assets | Generic Node install exists; hermetic monorepo build artifact contract absent | `PV-GAP-002` |
| SQLite/native Node module support | Runtime family exists; architecture-specific native dependency flow unverified | `PV-GAP-002` |
| Local-only operation without sync server | Plausible from upstream architecture; exact chosen entrypoint not yet pinned/tested | Source checkout required |
| Budget imports, exports, and downloaded files | Guest file broker absent | `PV-GAP-006` |
| Web workers/service workers/cookies/blob downloads across WebViews | Not assessed by current generic host tests | `PV-GAP-009` |
| Optional bank/sync egress | Per-destination egress permissions absent | `PV-GAP-005` |

### Verdict

`NO_GO_HOST_GAP`. After generic runtime artifacts, file brokering, and WebView conformance exist, re-run the gate against a pinned Actual release and choose between its local desktop core and server topology. Do not maintain an Actual-specific fork inside trusted host code.

Adapter effort after closure: **large** because of the monorepo and desktop/server boundary. Reusable host effort: **large**.

## 3. Baby Buddy — private household vault

### Application evidence

Baby Buddy is a Python/Django application with a built frontend/static-asset pipeline, database migrations, reports, API integrations, localization, and production deployment configuration. SQLite has historically been usable for small deployments, while current production recommendations and exact dependency constraints must be confirmed at a pinned revision. Media/static paths and the chosen production WSGI/ASGI server must also be established from source.

### Preliminary capability matrix

| Requirement | Current PixieVault state | Disposition |
| --- | --- | --- |
| Exact Python runtime and pinned native wheels on all targets | Python virtualenv/pip support exists; hermetic versions/wheels are unverified | `PV-GAP-002` |
| One-shot database migration before readiness | Can be hidden in a shell/start script, but no generic migration lifecycle/rollback contract | `PV-GAP-004` |
| Production local HTTP server and assigned port | Generic sidecar support exists; app command and Windows behavior require proof | Condition after source checkout |
| Database and uploaded media persistence | Mutable data env exists; file-tree lock/backup semantics need proof | `PV-GAP-007` |
| Static asset build without Node at install time | Build-time packaging is possible in principle; artifact completeness needs audit | Condition after source checkout |
| Import/export and optional API integrations | File broker and scoped egress absent | `PV-GAP-005`, `PV-GAP-006` |

### Verdict

`NO_GO_HOST_GAP`. Baby Buddy is likely the easiest Python representative once hermetic Python artifacts, migration lifecycle, file brokering, and encrypted mutable-tree behavior are implemented and verified.

Adapter effort after closure: **medium**. Reusable host effort: **large**.

## 4. Paperless-ngx — private document vault

### Application evidence

Paperless-ngx is a multi-process document management system rather than a single web sidecar. Its core value depends on ingestion, background task processing, OCR, PDF/image conversion, indexing, document/media storage, database migrations, and a web UI. Normal deployment uses container-installed native tools and additional services. Optional Tika/Gotenberg integrations add further processes but may be excluded if equivalent core document formats remain supported.

### Preliminary capability matrix

| Requirement | Current PixieVault state | Disposition |
| --- | --- | --- |
| Web plus durable workers, scheduler, broker, migrations, ordered shutdown | Multiple generic processes exist, but roles/dependencies/restarts/durable jobs are not modeled | `PV-GAP-004`, `PV-GAP-010` |
| OCR/PDF/image native executables and libraries | Binary/custom types exist; portable licensed artifact bundles and version resolution are absent | `PV-GAP-003` |
| Consumption directory and filesystem watcher | Arbitrary writable dirs are not a safe ingestion contract | `PV-GAP-011` |
| Large originals, archive files, thumbnails, indexes, logs, and temp data | Full encrypted file-tree lifecycle/quotas/snapshot consistency not established | `PV-GAP-007` |
| Database and broker backup consistency | No infrastructure-service backup transaction contract | `PV-GAP-010` |
| File import/export, downloads, embedded PDFs | File broker and WebView conformance missing | `PV-GAP-006`, `PV-GAP-009` |

### Verdict

`NO_GO_HOST_GAP`. Keep Paperless-ngx as the capstone test. Attempting it first would encourage Docker emulation or app-specific orchestration inside PixieVault. It becomes a sound target only after generic workers, infrastructure services, native tool bundles, ingestion staging, and large encrypted file trees are product capabilities.

Adapter effort after closure: **large**. Reusable host effort: **extra large**.

## Recommended platform sequence

1. **Make the gate truthful:** close `PV-GAP-001`; add a machine-readable host capability inventory tied to code and tests.
2. **Establish the single-sidecar class:** close `PV-GAP-002`, `006`, `007`, and `008`; re-run Allos and Baby Buddy.
3. **Establish rich desktop-web compatibility:** close `PV-GAP-009`; re-run Actual Budget.
4. **Establish controlled connectivity:** close `PV-GAP-005` before enabling any bank, health, email, webhook, scraping, or cloud integration.
5. **Establish multi-process document systems:** close `PV-GAP-003`, `004`, `010`, and `011`; then re-run Paperless-ngx.

The target product classes become:

- static/bridge apps;
- one local web sidecar plus SQLite/files;
- local web sidecar plus bounded scheduler/worker;
- multi-service applications with managed native tools and durable job infrastructure.

That taxonomy is more durable than accumulating framework-specific support.

## Next action

Pin the current PixieVault commit and one upstream release/commit for each application, check out the sources, and run `PRE_MIGRATION_PROMPT.md` as four independent audits. The first engineering slice should be `PV-GAP-001` plus a design for `PV-GAP-002` and `PV-GAP-007`; those three determine whether PixieVault can honestly claim portable, encrypted sidecar applications.
