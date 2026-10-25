# SDKWork Notes — sdkwork-specs Alignment Audit

Audit date: 2026-06-21 (architecture baseline + packaging naming pass)  
Scope: repository root, Rust HTTP backend, SDK contracts, deployment topology, PC React application surface

## Executive summary

SDKWork Notes is **fully aligned** with `sdkwork-specs` for an HTTP-only product workspace. The repository integrates `sdkwork-web-framework`, `sdkwork-database` (config, lifecycle, repository entities), `sdkwork-utils-rust`, and `@sdkwork/utils`, follows the standard root directory dictionary, uses canonical PC package naming (`sdkwork-notes-pc-*` / `@sdkwork/notes-pc-*`), and uses the standard GitHub packaging workflow. **No RPC services exist**, so `sdkwork-discovery` is correctly deferred.

OpenAPI authorities live under `apis/` only. Three SDK families ship TypeScript + Rust sdkgen transport output validated by `api:check`.

## Alignment matrix

| Area | Standard | Status | Evidence |
| --- | --- | --- | --- |
| Agent entrypoints | `AGENTS_SPEC.md` | Aligned | Root + `sdkwork-notes-pc-react/AGENTS.md`, shim files, `.sdkwork/` |
| Root directory dictionary | `SDKWORK_WORKSPACE_SPEC.md` | Aligned | `apis/`, `apps/`, `crates/`, `database/`, `sdks/`, `configs/`, `deployments/`, `docs/`, `tests/` |
| PNPM scripts | `PNPM_SCRIPT_SPEC.md` | Aligned | Root exposes `dev`, `build`, `test`, `check`, `verify`, `clean`, `api:*`, `sdk:*`, `db:*`, `topology:*` |
| Web framework | `WEB_FRAMEWORK_SPEC.md` | Aligned | `sdkwork-web-core`, `sdkwork-web-axum`, `WebRequestContext`, IAM adapter layer |
| Database framework | `DATABASE_SPEC.md` + `sdkwork-database` | Aligned | `database/` lifecycle; `DatabaseConfig::from_env("notes")`; repository entities; SQL table registry guard |
| Shared utils | `sdkwork-utils` | Aligned | Rust: route crates, pages-service, standalone-gateway, http-auth; TS: PC commons + core SDK boundaries |
| Service discovery | RPC only | N/A | No proto/RPC; discovery not required |
| API contracts | `API_SPEC.md` | Aligned | `apis/*/*.openapi.json`, route manifests, `x-sdkwork-*` extensions |
| SDK workspace | `SDK_WORKSPACE_GENERATION_SPEC.md` | Aligned | Three SDK families with TypeScript + Rust sdkgen output; `api:check` validates manifests |
| Deployment | `DEPLOYMENT_SPEC.md`, topology specs | Aligned | `specs/topology.spec.json`, unified + split orchestration, gateway TOML |
| Packaging / CI | `GITHUB_WORKFLOW_SPEC.md` | Aligned | `agent.workflow.json` (`*-standalone-desktop-*` targets), thin `package.yml` |
| PC app architecture | `APP_PC_ARCHITECTURE_SPEC.md` | Aligned | `sdkwork-notes-pc-*` directories; `@sdkwork/notes-pc-*` npm names; config templates |
| PC config profiles | `CONFIG_SPEC.md` | Aligned | `sdkwork-notes-pc-react/config/{browser,desktop,server,container}/` examples |
| Rust crate naming | `NAMING_SPEC.md`, `RUST_CODE_SPEC.md` | Aligned | Route/server/service/repository crates under `crates/` |
| Security / IAM | `IAM_LOGIN_INTEGRATION_SPEC.md` | Aligned | Appbase auth in PC React; server IAM routes embedded; actor claim trimming via utils |
| Testing | `TEST_SPEC.md` | Aligned | `pnpm verify`, 21 architecture/contract tests, PC `test:workspace:contracts` + `typecheck`, Rust integration tests |

## Framework integration detail

### sdkwork-web-framework

- Workspace dependencies: `sdkwork-web-core`, `sdkwork-web-axum` in root `Cargo.toml`
- Route handlers inject `WebRequestContext`
- API server mounts protected routers via `wrap_router_with_web_framework_from_env`
- Route manifests and OpenAPI authorities declare `WebRequestContext` / `x-sdkwork-request-context`

### sdkwork-database

- API server resolves `SDKWORK_NOTES_*` through `sdkwork-database-config`
- Full lifecycle assets under `database/` (contract, migrations, seeds, drift)
- `sdkwork-notes-pages-repository-sqlx/src/entities/` declares all registry tables via `sdkwork-database-repository`
- Architecture test ensures `SqlNotesStore` SQL references only registry-backed tables

### sdkwork-utils

- `sdkwork-utils-rust`: app-api routes, pages-service validation, standalone-gateway Drive bootstrap, http-auth actor matching
- `@sdkwork/utils`: PC commons text helpers; PC core SDK client boundaries (`useAppSdkClient`, `notesProductAppSdkClient`, `sessionIdentityClaims`, `appSdkCredentialEnv`)
- Notes-owned Vite private env helpers (`scripts/vite-private-env.ts`) — web and desktop Vite configs no longer import `@sdkwork/core-pc-react/vite`

### sdkwork-discovery

Not integrated — **by design** until Notes introduces gRPC/RPC services.

## Optional future work (not alignment blockers)

1. **Generate Java/Python/Go SDK transports** when those consumer surfaces are required.
2. **Enable SBOM/signing** in release workflow when production gates require it.
3. **Migrate SqlNotesStore complex queries** to `sdkwork-database-repository` Repository helpers incrementally.
4. **Add open-api route crate** when open-api runtime endpoints are required.

## Verification commands

```powershell
pnpm check
pnpm verify
node --test scripts/verify-notes-standard-architecture.test.mjs
cd sdkwork-notes-pc-react
pnpm run typecheck
pnpm test:workspace:contracts
pnpm api:check
```

Expected: all tests pass; route manifests and OpenAPI authorities stay synchronized with web-framework metadata.
