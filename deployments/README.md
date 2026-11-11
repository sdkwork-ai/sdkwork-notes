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

## Drive integration

Notes reaches sdkwork-drive **in-process** by default (MODULE_SPEC.md §4.2):
`InProcessDrivePageContentPort` calls `sdkwork-drive-workspace-service`,
`sdkwork-drive-object-runtime`, and `sdkwork-drive-storage-contract` on the host
PostgreSQL pool, so a deployment whose process owns that pool needs no Drive
endpoint and no service credential.

The Notes gateway container is its own process and does not carry the Drive
storage configuration, so the shipped topology profiles choose their transport
explicitly:

- `SDKWORK_NOTES_DRIVE_MODE=http` plus `SDKWORK_DRIVE_FACADE_URL` — split
  deployment; the endpoint is validated at startup. Used by
  `cloud.production`, `cloud.demo`, and `standalone.production`.
- `SDKWORK_NOTES_DRIVE_MODE=memory` — local/in-memory Drive. Used by
  development, test, and staging profiles.
- `SDKWORK_NOTES_DRIVE_MODE=in-process` (the default) — call Drive in this
  process on the host PostgreSQL pool.
