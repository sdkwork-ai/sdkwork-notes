# deployments/

Deployment manifests, release topology handoff, and environment-specific deployment profiles for SDKWork Notes.

## Deploy manifest

Per `SDKWORK_DEPLOY_SPEC.md`, this repository owns a single deployment contract:

- [deploy.yaml](deploy.yaml) — multi-profile deploy manifest (`cloud.production` default)

Validate:

```bash
pnpm deploy:validate
pnpm deploy:plan
```

## Topology and gateway

Current deployment topology is declared in:

- [../specs/topology.spec.json](../specs/topology.spec.json)
- [../etc/topology/](../etc/topology/)
- Platform ingress configuration is owned by the platform deployment authority; Notes publishes `sdkwork-api-notes-assembly`.

## Packaging

GitHub packaging uses [../agent.workflow.json](../agent.workflow.json) and [../.github/workflows/package.yml](../.github/workflows/package.yml).

Desktop release targets are declared in `agent.workflow.json` and referenced from `deploy.yaml` `packages` (`desktop-windows`, `desktop-macos`, `desktop-linux`).

## Production Drive integration

Production topology profiles set:

- `SDKWORK_NOTES_USE_MEMORY_DRIVE=0`
- `SDKWORK_DRIVE_FACADE_URL=https://api.sdkwork.com`

Local development defaults to in-memory Drive unless `SDKWORK_NOTES_USE_MEMORY_DRIVE=0` and `SDKWORK_DRIVE_FACADE_URL` are set.
