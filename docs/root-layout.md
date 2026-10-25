# SDKWork Notes Root Layout

This repository is a **multi-surface product workspace** governed by `sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`. It combines a Rust HTTP backend, SDK contract workspace, topology configs, and client application roots.

## Active standard directories

| Directory | Status | Purpose |
| --- | --- | --- |
| `apis/` | Active | Author-owned OpenAPI authorities |
| `apps/` | Pointer | Application surface roots (see below) |
| `crates/` | Active | Rust route crates and API server |
| `configs/` | Active | Topology profiles and gateway TOML |
| `deployments/` | Reserved | Deployment manifests and release topology handoff |
| `docs/` | Active | Architecture, schema registry, release notes |
| `examples/` | Reserved | Runnable examples |
| `jobs/` | Reserved | Background workers |
| `plugins/` | Reserved | Repository-local plugins |
| `scripts/` | Active | Dev, topology, contract verification |
| `sdks/` | Active | SDK families and route manifests |
| `services/` | Active | Rust product/service crates |
| `specs/` | Active | Root topology and component contracts |
| `tests/` | Pointer | Cross-cutting verification entry (see below) |
| `tools/` | Reserved | Maintainer tooling |

## Intentionally absent or relocated

| Standard path | Notes decision |
| --- | --- |
| `apps/<product>-pc/` | PC React/Tauri app lives at `sdkwork-notes-pc-react/` for historical layout; `apps/README.md` documents the mapping. |
| Root `agent.app.config.json` | Application identity lives in `sdkwork-notes-pc-react/agent.app.config.json`. |
| RPC / `sdkwork-discovery` | No gRPC services yet; discovery deferred until RPC is introduced. |

## Architecture-specific paths (not project-root dictionary)

These belong to the PC application surface under `sdkwork-notes-pc-react/`:

- `packages/` — PC React feature packages
- `src/` — thin app bootstrap
- `contracts/` — remote-apply contract inputs

## Framework integration summary

| Framework | Integration |
| --- | --- |
| `sdkwork-web-framework` | Route crates use `WebRequestContext`; API server mounts IAM + Notes routes through `sdkwork-web-axum` and `sdkwork-iam-web-adapter`. |
| `sdkwork-database` | API server resolves `SDKWORK_NOTES_*` database config through `sdkwork-database-config`; SQLx persistence in product service. |
| `sdkwork-discovery` | Not required (HTTP-only, no RPC). |

Verification: `pnpm verify` and `node --test scripts/verify-notes-standard-architecture.test.mjs`.
