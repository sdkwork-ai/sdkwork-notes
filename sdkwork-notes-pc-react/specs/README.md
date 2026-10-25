# SDKWork Notes PC React Component Specs

This directory is the local standards index for `sdkwork-notes-pc-react`.

Root SDKWork standards remain authoritative. Local component specs can narrow or document this component, but they must not contradict [the root standards](../../../../specs/README.md).

## Component

| Field | Value |
| --- | --- |
| Name | `sdkwork-notes-pc-react` |
| Type | `react-tauri-app` |
| Root | `sdkwork-notes/sdkwork-notes-pc-react` |
| Domain | `content` |
| Capability | `notes` |
| Languages | `typescript` |
| Status | `ACTIVE` |

## Contract Manifest

- [component.spec.json](./component.spec.json) is the machine-readable component contract.
- Consumers should integrate through public exports, runtime entrypoints, SDK clients, or adapters declared in the manifest.
- Generated SDK language outputs are represented at their SDK family root instead of duplicating local specs in generated folders.

## Canonical Specs

| Spec | Applies Because |
| --- | --- |
| [APP_MANIFEST_SPEC.md](../../../../specs/APP_MANIFEST_SPEC.md) | agent.app.config.json application registration rules. |
| [APPLICATION_SPEC.md](../../../../specs/APPLICATION_SPEC.md) | Application shell and module composition. |
| [COMPONENT_SPEC.md](../../../../specs/COMPONENT_SPEC.md) | Local component specs directory and manifest rules. |
| [CONFIG_SPEC.md](../../../../specs/CONFIG_SPEC.md) | Runtime configuration, environment, SDK bootstrap, and feature flag rules. |
| [DEPLOYMENT_SPEC.md](../../../../specs/DEPLOYMENT_SPEC.md) | SaaS/private/local runtime parity and deployment rules. |
| [DOCUMENTATION_SPEC.md](../../../../specs/DOCUMENTATION_SPEC.md) | Module README, examples, ADR, changelog, and runbook rules. |
| [DOMAIN_SPEC.md](../../../../specs/DOMAIN_SPEC.md) | Canonical domain ownership and naming. |
| [FRONTEND_SPEC.md](../../../../specs/FRONTEND_SPEC.md) | UI, service, SDK, accessibility, and frontend runtime rules. |
| [GOVERNANCE_SPEC.md](../../../../specs/GOVERNANCE_SPEC.md) | Standard ownership, exception, compatibility, and migration rules. |
| [I18N_SPEC.md](../../../../specs/I18N_SPEC.md) | User-facing language, locale, message catalog, and fallback rules. |
| [MODULE_SPEC.md](../../../../specs/MODULE_SPEC.md) | Reusable package contract and dependency direction. |
| [OBSERVABILITY_SPEC.md](../../../../specs/OBSERVABILITY_SPEC.md) | Log, metric, trace, audit, and diagnostic rules. |
| [PERFORMANCE_SPEC.md](../../../../specs/PERFORMANCE_SPEC.md) | Latency, pagination, bundle, scalability, and retry budget rules. |
| [README.md](../../../../specs/README.md) | SDKWork root standards entrypoint. |
| [SDK_SPEC.md](../../../../specs/SDK_SPEC.md) | SDK generation and SDK integration rules. |
| [TEST_SPEC.md](../../../../specs/TEST_SPEC.md) | Contract, frontend, SDK, security, parity, and documentation verification rules. |

## Public Exports

- Public exports are not declared in the package manifest.

## SDK Clients

- No generated SDK client class is declared at this component boundary.

## Local Extension Specs

- No local extension specs are declared yet.

## Verification

- `pnpm --filter @sdkwork/notes-pc-react typecheck`
