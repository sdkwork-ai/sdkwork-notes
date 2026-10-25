# apps/

Application: notes
Status: active
Owner: SDKWork maintainers
Specs: APPLICATION_SPEC.md, SDKWORK_WORKSPACE_SPEC.md

## Primary App Surface

The repository root is not the primary runnable app surface.
Notes uses a hybrid layout until migration to `apps/sdkwork-notes-pc/`:

- `sdkwork-notes-pc-react/` with `sdkwork-notes-pc-react/agent.app.config.json`

Repository-root `pnpm-workspace.yaml` owns sibling package paths; do not add nested workspace files under client app roots.

## Directory Index

| Directory | Surface role | Runnable | Purpose | Entry |
| --- | --- | --- | --- | --- |
| sdkwork-notes-pc-react | pc-react | yes | Primary PC React + Tauri application | [README](../sdkwork-notes-pc-react/README.md) |

Local-only prototypes under `apps/sdkwork-notes-pc/` are gitignored and must not be treated as production surfaces.

## Allowed Content

- Selected language/architecture application roots with `README.md`, `AGENTS.md`, `.sdkwork/`, and `specs/` when authored packages exist.
- Architecture-local `packages/`, `config/`, `src/`, `lib/`, `App/`, or `entry/` directories required by the owning architecture standard.

## Forbidden Content

- Repository-root API contracts, generated SDK workspaces, Rust crates, or deployment descriptors moved under `apps/`.
- Runtime secrets, user-private state, generated SDK transport output, or cross-application copied business logic.
- Nested `pnpm-workspace.yaml` files under client app roots.

## Related Specs

- `../sdkwork-specs/APPLICATION_SPEC.md`
- `../sdkwork-specs/SDKWORK_WORKSPACE_SPEC.md`
- `../sdkwork-specs/APP_CLIENT_ARCHITECTURE_ALIGNMENT_SPEC.md`
- `../sdkwork-specs/APP_COMPOSITION_SPEC.md`

## Verification

```bash
pnpm run check:app-composition
node ../sdkwork-specs/tools/check-apps-directory-index.mjs --root .
```
