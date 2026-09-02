# PixieVault Pre-Migration Compatibility Gate

Use this prompt before changing either the source application or PixieVault. Its purpose is to decide whether a faithful migration is supported by the **implemented and verified** host contract, identify reusable host gaps, and prevent app-specific workarounds from quietly becoming architecture.

This is a read-only assessment. A `GO` authorizes planning, not migration. Run `MIGRATION_PROMPT.md` only after this gate returns `GO` or the accepted conditions of a `CONDITIONAL_GO` have been met.

````markdown
You are assessing an existing application for faithful migration into PixieVault. Do not edit the source application, PixieVault, manifests, lockfiles, or generated assets during this assessment.

## Inputs

- Source application or repository: `<SOURCE>`
- Source revision: `<FULL_COMMIT_SHA_OR_RELEASE_TAG>`
- PixieVault repository: `<PIXIEVAULT_REPOSITORY>`
- PixieVault revision/version: `<FULL_COMMIT_SHA_AND_VERSION>`
- Required targets: `<WINDOWS_LINUX_MACOS>`
- Required feature scope: `<FEATURES_THAT_MUST_REMAIN_FAITHFUL>`
- Allowed exclusions: `<EXPLICITLY_ACCEPTED_OPTIONAL_FEATURES_OR_NONE>`
- Operating mode: offline after installation unless explicitly stated otherwise

## Decision rule

Judge compatibility against code and tests at the supplied PixieVault revision, not roadmap language or examples alone.

Classify every claimed PixieVault capability as:

- `IMPLEMENTED_VERIFIED`: present in host code and exercised by a relevant automated or native-host test.
- `IMPLEMENTED_UNVERIFIED`: present in code but not verified for the required target or behavior.
- `DOCUMENTED_ONLY`: promised by documentation but missing, materially inconsistent, or unenforced in code.
- `ABSENT`: neither implemented nor documented sufficiently for safe use.

Use exactly one verdict:

- `GO`: all required behavior is supported with no source changes beyond configuration, packaging, path/port adapters, a health endpoint, or build-time asset preparation; every security-critical dependency is `IMPLEMENTED_VERIFIED` on every required target.
- `CONDITIONAL_GO`: the application can be migrated faithfully without changing PixieVault architecture, but named, bounded application adapters or verification work are required. List each condition as a pass/fail acceptance test. Do not use this verdict for missing host capabilities.
- `NO_GO_HOST_GAP`: faithful migration requires one or more reusable PixieVault capabilities that are absent, documented-only, or unverified where security or data integrity depends on them.
- `NO_GO_APP`: migration would require an unacceptable product change, prohibited service, incompatible license/distribution model, unavoidable remote browser/UI, or removal of required behavior.
- `INSUFFICIENT_EVIDENCE`: required source, revision, license, build, runtime, or deployment evidence could not be inspected. Never turn uncertainty into a GO.

A reduced demo is not a faithful migration. Optional features may be excluded only when the input explicitly allows it and the core data model and workflows remain intact.

## Required inspection

### 1. Pin facts and instructions

Record exact revisions. Read all repository-level agent/contributor instructions that govern inspected files. Inspect primary source and official project documentation rather than summaries. Record the license and redistribution obligations. Mark mutable upstream facts with the inspection date.

### 2. Inventory the complete process topology

List every frontend, HTTP service, worker, scheduler, queue, broker, database, search engine, OCR/conversion process, watcher, and optional integration. For each process record:

- runtime and exact version constraint;
- command, working directory, bind address, port behavior, and readiness behavior;
- whether it must run continuously or only while the PixieVault app is open;
- shutdown and child-process behavior;
- required native libraries, executables, compiler toolchains, package managers, and platform-specific artifacts;
- inter-process protocols and startup ordering.

Do not treat a Docker image as a runtime specification. Decompose what the image installs and launches.

### 3. Inventory build and supply-chain requirements

Record lockfiles, workspace tooling, generated code, frontend compilation, database migrations, native modules/wheels, post-install scripts, runtime downloads, browser downloads, and architecture-specific artifacts.

Prove how installation and first launch work without an undeclared compiler, shell utility, container engine, administrator privilege, CDN, package registry, or network connection. If PixieVault cannot package or provision a pinned dependency reproducibly on every target, record a host gap.

### 4. Inventory persistence and lifecycle

List every database, upload, media file, thumbnail, index, cache, log, secret, session, backup, temporary file, socket, and generated artifact. For each item record:

- creation time and writer process;
- required durability;
- required filesystem semantics, locking, atomic rename, memory mapping, WAL, or watcher behavior;
- expected size and growth;
- backup/restore consistency requirements;
- whether it contains sensitive information even if described upstream as a cache or log.

Map every mutable path to an implemented PixieVault data-path contract. App resources are immutable. Determine what happens on vault lock, app stop, crash, failed migration, upgrade, and rollback. A database merely residing under `VAULT_APP_DATA` is not proof that it is encrypted while the vault is locked; verify the actual materialization/encryption lifecycle.

### 5. Inventory privileges and external boundaries

List all outbound hosts/protocols, inbound listeners, webhooks, mobile clients, SMTP, OAuth/OIDC callbacks, bank/health APIs, AI providers, update checks, telemetry, remote fonts/assets, URL scraping, and map tiles. Also list file pickers, drag/drop, scanners, cameras, clipboard, printing, notifications, external navigation, and custom URL schemes.

For each boundary require a least-privilege, user-visible PixieVault permission with enforceable scope. Configuration or an environment variable is not a sandbox permission. Flag any application authentication assumption that changes when the service is reachable only through a local WebView.

### 6. Inspect rendering assumptions

Check SPA routing, service workers, WebSockets/SSE, cookies, secure-context requirements, CSP, cross-origin requests, popup/new-window behavior, downloads, blobs, embedded PDFs, media, WebAssembly, workers, dynamic imports, locale/Unicode, packaged fonts/icons, and WebView2/WKWebView/WebKitGTK differences.

### 7. Compare against the real PixieVault contract

Inspect at minimum:

- manifest Rust types and validation;
- package builder, extractor, and immutable-resource rules;
- Composer provisioning, service startup, health checks, sandbox enforcement, log draining, and teardown;
- runtime discovery/version enforcement for every target;
- mutable-data environment injection and vault lock/unlock behavior;
- bridge APIs and Tauri permissions;
- native-host and cross-platform tests.

For every required capability provide code and test evidence. If documentation conflicts with code, code controls the compatibility verdict and the conflict is a separate defect.

## Architecture fitness rules

An application adapter is acceptable when it translates an existing generic host contract: assigned loopback port, data directory, health endpoint, packaged frontend, or a supported import/export broker.

A host change is a reusable capability when at least two plausible applications could need it, including runtime families, dependency artifacts, workers, scheduled jobs, native tools, databases/brokers, file ingress, egress permissions, secrets, notifications, or lifecycle hooks. Classify it as a host gap; do not hide it in an application adapter.

An application-specific hack includes hard-coded knowledge of an app ID, framework, path layout, migration command, database schema, or third-party domain in trusted PixieVault code. Any required hack produces `NO_GO_HOST_GAP` until replaced by a declarative generic capability.

## Required output

Return these sections in order:

1. `Verdict` — one allowed verdict, one-sentence rationale, assessed revisions, date, targets, required scope, and accepted exclusions.
2. `Evidence quality` — sources inspected, missing evidence, and all documentation/code conflicts.
3. `Application topology` — processes, runtimes, dependencies, startup ordering, and lifecycle.
4. `Capability matrix` — one row per requirement with columns: requirement, application evidence, PixieVault code evidence, PixieVault test evidence, capability state, target coverage, disposition.
5. `Storage and security map` — every mutable path and external boundary with its proposed host contract and lock/stop behavior.
6. `Feature-parity boundary` — required, optional, excluded, and impossible features. No silent exclusions.
7. `Conditions` — for `CONDITIONAL_GO`, numbered binary acceptance tests with an owner. Otherwise write `Not applicable`.
8. `Blocking gaps` — gap ID, generic capability, affected apps/classes, security reason, smallest declarative host contract, and proof required to close it.
9. `Migration shape` — static, bridge adapter, Composer sidecar, or hybrid; proposed services and manifest concepts. Do not produce a final manifest.
10. `Effort and risk` — application-adapter effort separately from reusable PixieVault work; use relative sizes with explicit assumptions, not invented calendar estimates.
11. `Next action` — either run the migration prompt, satisfy the listed conditions, implement/verify host gaps, reject the app, or gather missing evidence.

Finish with a machine-readable fenced JSON block using this schema:

```json
{
  "schema_version": "1",
  "verdict": "GO|CONDITIONAL_GO|NO_GO_HOST_GAP|NO_GO_APP|INSUFFICIENT_EVIDENCE",
  "source_revision": "string",
  "pixievault_revision": "string",
  "targets": ["windows", "linux", "macos"],
  "required_scope": ["string"],
  "accepted_exclusions": ["string"],
  "conditions": [{"id": "C-001", "test": "string", "owner": "app|pixievault"}],
  "gaps": [{"id": "PV-GAP-001", "capability": "string", "state": "IMPLEMENTED_UNVERIFIED|DOCUMENTED_ONLY|ABSENT", "targets": ["string"], "blocks": ["string"]}],
  "documentation_conflicts": [{"claim": "string", "code_evidence": "string"}],
  "next_action": "string"
}
```
````

## Portfolio use

Run the prompt independently for each application at a pinned revision. Aggregate gaps by stable gap ID and generic capability, not by application name. A gap is closed only when its declarative contract, enforcement, packaging behavior, lifecycle behavior, and required target tests all exist.
