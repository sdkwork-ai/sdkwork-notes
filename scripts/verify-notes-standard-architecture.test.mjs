import assert from 'node:assert/strict';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const ROOT = process.cwd();

const STANDARD_ROOT_DIRECTORIES = [
  'apis',
  'apps',
  'crates',
  'database',
  'sdks',
  'jobs',
  'tools',
  'plugins',
  'examples',
  'configs',
  'deployments',
  'scripts',
  'docs',
  'tests',
];

const REQUIRED_WORKSPACE_FILES = [
  'AGENTS.md',
  'CLAUDE.md',
  'CODEX.md',
  'GEMINI.md',
  'README.md',
  'Cargo.toml',
  'agent.workflow.json',
  '.github/workflows/package.yml',
  '.sdkwork/README.md',
  '.sdkwork/skills/README.md',
  '.sdkwork/plugins/README.md',
  'docs/root-layout.md',
];

const API_INPUTS = [
  'apis/app-api/notes/notes-app-api.openapi.json',
  'apis/backend-api/notes/notes-backend-api.openapi.json',
  'apis/open-api/notes/notes-open-api.openapi.json',
];

const ROUTE_MANIFESTS = [
  {
    file: 'sdks/_route-manifests/app-api/sdkwork-routes-notes-app-api.route-manifest.json',
    apiSurface: 'app-api',
  },
  {
    file: 'sdks/_route-manifests/backend-api/sdkwork-routes-notes-backend-api.route-manifest.json',
    apiSurface: 'backend-api',
  },
];

const WEB_FRAMEWORK_CRATES = [
  'crates/sdkwork-routes-notes-app-api/Cargo.toml',
  'crates/sdkwork-routes-notes-backend-api/Cargo.toml',
  'crates/sdkwork-routes-notes-http-auth/Cargo.toml',
  'crates/sdkwork-api-notes-standalone-gateway/Cargo.toml',
];

function read(relativePath) {
  return readFileSync(path.join(ROOT, relativePath), 'utf8');
}

function readJson(relativePath) {
  const text = read(relativePath).replace(/^\uFEFF/u, '');
  return JSON.parse(text);
}

function exists(relativePath) {
  return existsSync(path.join(ROOT, relativePath));
}

test('declares SDKWork standard root directory dictionary', () => {
  for (const directory of STANDARD_ROOT_DIRECTORIES) {
    assert.equal(exists(directory), true, `${directory}/ should exist`);
    assert.equal(exists(path.join(directory, 'README.md')), true, `${directory}/README.md should exist`);
  }
});

test('declares workspace agent entrypoints and packaging workflow', () => {
  for (const file of REQUIRED_WORKSPACE_FILES) {
    assert.equal(exists(file), true, `${file} should exist`);
  }

  const workflow = readJson('agent.workflow.json');
  assert.equal(workflow.app.id, 'sdkwork-notes-pc-react');
  assert.equal(workflow.app.configPath, 'sdkwork-notes-pc-react/agent.app.config.json');
});

test('declares author-owned API contracts under apis/', () => {
  for (const file of API_INPUTS) {
    assert.equal(exists(file), true, `${file} should exist`);
    const openapi = readJson(file);
    assert.equal(openapi.openapi, '3.1.2');
  }
});

test('integrates sdkwork-web-framework in HTTP route crates and standalone gateway', () => {
  const rootCargo = read('Cargo.toml');
  assert.match(rootCargo, /sdkwork-web-core/);
  assert.match(rootCargo, /sdkwork-web-axum/);

  for (const cargoPath of WEB_FRAMEWORK_CRATES) {
    const cargo = read(cargoPath);
    assert.match(cargo, /sdkwork-web-/);
  }

  const authBootstrap = read('crates/sdkwork-api-notes-standalone-gateway/src/bootstrap/auth.rs');
  assert.match(authBootstrap, /wrap_router_with_web_framework_from_env/);
});

test('integrates sdkwork-utils in HTTP route crates', () => {
  const rootCargo = read('Cargo.toml');
  assert.match(rootCargo, /sdkwork-utils-rust/);

  const appRouteCargo = read('crates/sdkwork-routes-notes-app-api/Cargo.toml');
  assert.match(appRouteCargo, /sdkwork-utils-rust/);
});

test('integrates sdkwork-utils-typescript in PC React workspace', () => {
  const workspace = read('pnpm-workspace.yaml');
  assert.match(workspace, /sdkwork-utils-typescript/);

  const packageJson = readJson('sdkwork-notes-pc-react/package.json');
  assert.match(packageJson.dependencies['@sdkwork/utils'], /workspace:\*/);
});

test('consumes @sdkwork/utils from shared PC commons text helpers', () => {
  const textUtils = read('sdkwork-notes-pc-react/packages/sdkwork-notes-pc-commons/src/utils/text.ts');
  assert.match(textUtils, /@sdkwork\/utils/);
  assert.match(textUtils, /isBlank/);
  assert.match(textUtils, /trim/);
});

test('integrates sdkwork-utils-rust in product service validation', () => {
  const serviceCargo = read('crates/sdkwork-notes-pages-service/Cargo.toml');
  assert.match(serviceCargo, /sdkwork-utils-rust/);

  const serviceSource = read('crates/sdkwork-notes-pages-service/src/service.rs');
  assert.match(serviceSource, /sdkwork_utils_rust::string::\{is_blank/);
});

test('integrates sdkwork-database in standalone gateway bootstrap', () => {
  const gatewayCargo = read('crates/sdkwork-api-notes-standalone-gateway/Cargo.toml');
  assert.match(gatewayCargo, /sdkwork-database-config/);
  assert.match(gatewayCargo, /sdkwork-database-sqlx|sdkwork_database_sqlx/);

  const databaseBootstrap = read('crates/sdkwork-api-notes-standalone-gateway/src/bootstrap/database.rs');
  assert.match(databaseBootstrap, /DatabaseConfig::from_env\("notes"\)/);
});

test('integrates sdkwork-database-repository entities in pages repository crate', () => {
  const repositoryCargo = read('crates/sdkwork-notes-pages-repository-sqlx/Cargo.toml');
  assert.match(repositoryCargo, /sdkwork-database-repository/);

  const entities = read('crates/sdkwork-notes-pages-repository-sqlx/src/entities/mod.rs');
  assert.match(entities, /sdkwork_database_repository::\{impl_entity_string_pk, Entity\}/);
  assert.match(entities, /notes_workspace/);
  assert.match(entities, /notes_page/);
  assert.match(entities, /notes_ai_job/);
});

test('declares canonical sdkgen transport output for typescript and rust SDK families', () => {
  const families = [
    {
      assembly: 'sdks/sdkwork-notes-app-sdk/sdk-manifest.json',
      typescriptManifest:
        'sdks/sdkwork-notes-app-sdk/sdkwork-notes-app-sdk-typescript/generated/server-openapi/package.json',
      rustManifest:
        'sdks/sdkwork-notes-app-sdk/sdkwork-notes-app-sdk-rust/generated/server-openapi/Cargo.toml',
    },
    {
      assembly: 'sdks/sdkwork-notes-backend-sdk/sdk-manifest.json',
      typescriptManifest:
        'sdks/sdkwork-notes-backend-sdk/sdkwork-notes-backend-sdk-typescript/generated/server-openapi/package.json',
      rustManifest:
        'sdks/sdkwork-notes-backend-sdk/sdkwork-notes-backend-sdk-rust/generated/server-openapi/Cargo.toml',
    },
    {
      assembly: 'sdks/sdkwork-notes-sdk/sdk-manifest.json',
      typescriptManifest:
        'sdks/sdkwork-notes-sdk/sdkwork-notes-sdk-typescript/generated/server-openapi/package.json',
      rustManifest: 'sdks/sdkwork-notes-sdk/sdkwork-notes-sdk-rust/generated/server-openapi/Cargo.toml',
    },
  ];

  for (const family of families) {
    assert.equal(exists(family.typescriptManifest), true, `${family.typescriptManifest} should exist`);
    assert.equal(exists(family.rustManifest), true, `${family.rustManifest} should exist`);

    const assembly = readJson(family.assembly);
    for (const language of ['typescript', 'rust']) {
      const entry = assembly.languages.find((item) => item.language === language);
      assert.equal(entry.generationState, 'generated', `${family.assembly} ${language} must be generated`);
    }
  }
});

test('PC React packages follow APP_PC_ARCHITECTURE_SPEC directory and npm naming', () => {
  const packagesDir = 'sdkwork-notes-pc-react/packages';
  const packageDirs = readdirSync(path.join(ROOT, packagesDir), { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);

  assert.ok(packageDirs.length > 0, `${packagesDir} should declare PC packages`);

  for (const directory of packageDirs) {
    assert.match(
      directory,
      /^sdkwork-notes-pc(-[a-z0-9-]+)?$/,
      `${directory} must follow sdkwork-notes-pc-* directory naming`,
    );

    const packageJson = readJson(`${packagesDir}/${directory}/package.json`);
    assert.match(
      packageJson.name,
      /^@sdkwork\/notes-pc(-[a-z0-9-]+)?$/,
      `${directory} npm name must follow @sdkwork/notes-pc-*`,
    );
  }
});

test('SqlNotesStore SQL references only database contract registry tables', () => {
  const store = read('crates/sdkwork-notes-pages-repository-sqlx/src/notes_store.rs');
  const registry = readJson('database/contract/table-registry.json');
  const registered = new Set(registry.tables.map((entry) => entry.table_name));
  const matches = store.matchAll(/\b(?:FROM|INTO|UPDATE|JOIN)\s+(notes_[a-z_]+)/gi);
  const referenced = new Set([...matches].map((match) => match[1]));

  for (const table of referenced) {
    assert.equal(
      registered.has(table),
      true,
      `notes_store.rs references ${table} which must exist in database/contract/table-registry.json`,
    );
  }
});

test('integrates sdkwork-utils in HTTP auth actor context checks', () => {
  const authCargo = read('crates/sdkwork-routes-notes-http-auth/Cargo.toml');
  assert.match(authCargo, /sdkwork-utils-rust/);

  const actorContext = read('crates/sdkwork-routes-notes-http-auth/src/actor_context.rs');
  assert.match(actorContext, /sdkwork_utils_rust::string::trim/);
});

test('does not declare sdkwork-discovery without RPC services', () => {
  const rootCargo = read('Cargo.toml');
  assert.doesNotMatch(rootCargo, /sdkwork-discovery/);

  const protoFiles = exists('apis/rpc') || exists('crates') && false;
  assert.equal(protoFiles, false);
  assert.equal(exists('apis/rpc'), false, 'RPC contracts should not exist yet');
});

test('route manifests declare WebRequestContext and apiSurface on every route', () => {
  for (const { file, apiSurface } of ROUTE_MANIFESTS) {
    const manifest = readJson(file);
    assert.ok(Array.isArray(manifest.routes) && manifest.routes.length > 0, `${file} should declare routes`);
    for (const route of manifest.routes) {
      assert.equal(route.requestContext, 'WebRequestContext', `${route.method} ${route.path} missing requestContext`);
      assert.equal(route.apiSurface, apiSurface, `${route.method} ${route.path} missing apiSurface`);
    }
  }
});

test('OpenAPI authorities declare x-sdkwork-request-context and x-sdkwork-api-surface', () => {
  const expectations = [
    { file: 'apis/app-api/notes/notes-app-api.openapi.json', apiSurface: 'app-api' },
    { file: 'apis/backend-api/notes/notes-backend-api.openapi.json', apiSurface: 'backend-api' },
    { file: 'apis/open-api/notes/notes-open-api.openapi.json', apiSurface: 'open-api' },
  ];

  const methods = new Set(['get', 'post', 'put', 'patch', 'delete']);

  for (const { file, apiSurface } of expectations) {
    const openapi = readJson(file);
    let operationCount = 0;
    for (const pathItem of Object.values(openapi.paths ?? {})) {
      for (const [method, operation] of Object.entries(pathItem ?? {})) {
        if (!methods.has(method)) {
          continue;
        }
        operationCount += 1;
        assert.equal(operation['x-sdkwork-request-context'], 'WebRequestContext', `${file} ${method} missing x-sdkwork-request-context`);
        assert.equal(operation['x-sdkwork-api-surface'], apiSurface, `${file} ${method} missing x-sdkwork-api-surface`);
      }
    }
    assert.ok(operationCount > 0, `${file} should contain HTTP operations`);
  }
});

test('declares topology and gateway deployment profiles', () => {
  assert.equal(exists('specs/topology.spec.json'), true);
  assert.equal(exists('etc/topology/standalone.development.env'), true);

  const topology = readJson('specs/topology.spec.json');
  assert.equal(topology.database?.appPrefix, 'SDKWORK_NOTES');
  assert.deepEqual(topology.surfaces?.['application.public-ingress']?.protocols, ['http']);
});

test('PC application root declares agent.app.config.json and component spec', () => {
  assert.equal(exists('sdkwork-notes-pc-react/agent.app.config.json'), true);
  assert.equal(exists('sdkwork-notes-pc-react/specs/component.spec.json'), true);
  assert.equal(exists('sdkwork-notes-pc-react/AGENTS.md'), true);

  const manifest = readJson('sdkwork-notes-pc-react/agent.app.config.json');
  assert.equal(manifest.schemaVersion, 3);
  assert.equal(manifest.kind, 'agent.app');
});

test('Rust HTTP crates follow sdkwork-routes-* and sdkwork-api-notes-standalone-gateway naming', () => {
  const expectedMembers = [
    'crates/sdkwork-routes-notes-app-api',
    'crates/sdkwork-routes-notes-backend-api',
    'crates/sdkwork-routes-notes-http-auth',
    'crates/sdkwork-api-notes-standalone-gateway',
    'crates/sdkwork-notes-pages-service',
    'crates/sdkwork-notes-pages-repository-sqlx',
  ];

  const cargo = read('Cargo.toml');
  for (const member of expectedMembers) {
    assert.match(cargo, new RegExp(`"${member.replaceAll('/', '\\/')}"`));
  }
});

test('Tauri desktop host component follows sdkwork-notes-pc-desktop naming', () => {
  const componentSpec = readJson(
    'sdkwork-notes-pc-react/packages/sdkwork-notes-pc-desktop/src-tauri/specs/component.spec.json',
  );
  assert.equal(componentSpec.component.name, 'sdkwork-notes-pc-desktop');

  const cargo = read('sdkwork-notes-pc-react/packages/sdkwork-notes-pc-desktop/src-tauri/Cargo.toml');
  assert.match(cargo, /^name = "sdkwork-notes-pc-desktop"/m);
});

test('PC vite configs use notes-owned private env helpers instead of core-pc-react', () => {
  const viteConfigs = [
    'sdkwork-notes-pc-react/vite.config.ts',
    'sdkwork-notes-pc-react/packages/sdkwork-notes-pc-desktop/vite.config.ts',
  ];

  for (const file of viteConfigs) {
    const source = read(file);
    assert.doesNotMatch(source, /@sdkwork\/core-pc-react/, `${file} must not import core-pc-react`);
    assert.match(source, /vite-private-env/, `${file} must import notes-owned vite-private-env helpers`);
  }

  assert.equal(exists('sdkwork-notes-pc-react/scripts/vite-private-env.ts'), true);
});

test('packaging workflow targets follow standalone desktop naming', () => {
  const workflow = readJson('agent.workflow.json');
  const targetIds = (workflow.targets ?? []).map((target) => target.id);

  assert.ok(targetIds.length > 0, 'agent.workflow.json should declare desktop release targets');

  for (const targetId of targetIds) {
    assert.match(
      targetId,
      /-standalone-desktop-/,
      `Expected ${targetId} to include standalone desktop profile segment`,
    );
    assert.match(
      workflow.targets.find((target) => target.id === targetId)?.outputGlobs?.join('\n') ?? '',
      /sdkwork-notes-pc-desktop/,
      `Expected ${targetId} artifact globs to reference sdkwork-notes-pc-desktop`,
    );
  }
});

test('schema registry documents Notes-owned tables', () => {
  assert.equal(exists('docs/schema-registry/README.md'), true);
  assert.equal(exists('docs/schema-registry/tables/001-notes-core.yaml'), true);
});

test('declares SDKWORK_DEPLOY_SPEC deployments/deploy.yaml', () => {
  assert.equal(exists('deployments/deploy.yaml'), true);
  const deploy = read('deployments/deploy.yaml');
  assert.match(deploy, /^version:\s*2/m);
  assert.match(deploy, /defaultProfile:\s*cloud\.production/);
  assert.match(deploy, /domain:\s*notes\.sdkwork\.com/);
});

test('wires deploy validation into pnpm check', () => {
  const packageJson = readJson('package.json');
  assert.match(packageJson.scripts.check, /check:deploy-standard/);
  assert.match(packageJson.scripts['deploy:validate'], /check-deploy-standard\.mjs/);
});

test('wires native app composition verification into pnpm check and verify', () => {
  const packageJson = readJson('package.json');
  assert.match(packageJson.scripts.check, /check:app-composition/);
  assert.match(packageJson.scripts.verify, /check:app-composition/);
  assert.match(packageJson.scripts['check:app-composition'], /verify-repo\.mjs/);
});

test('integrates sdkwork-drive through generated Rust app SDK facade', () => {
  const rootCargo = read('Cargo.toml');
  assert.match(rootCargo, /sdkwork_drive_app_sdk_generated_rust/);

  const facade = read('crates/sdkwork-api-notes-standalone-gateway/src/bootstrap/drive_app_sdk_facade.rs');
  assert.match(facade, /sdkwork_drive_app_sdk_generated_rust/);
  assert.match(facade, /uploader_uploads_prepare/);
  assert.match(facade, /upload_sessions_complete/);

  const appAssembly = readJson('sdks/sdkwork-notes-app-sdk/sdk-manifest.json');
  const driveDependency = appAssembly.sdkDependencies?.find(
    (item) => item.workspace === 'sdkwork-drive-app-sdk',
  );
  assert.ok(driveDependency, 'app SDK assembly must declare sdkwork-drive-app-sdk');
});

test('production topology profiles configure Drive facade instead of memory drive', () => {
  for (const profilePath of [
    'etc/topology/cloud.production.env',
    'etc/topology/standalone.production.env',
  ]) {
    const profileEnv = read(profilePath);
    assert.match(profileEnv, /SDKWORK_NOTES_USE_MEMORY_DRIVE=0/);
    assert.match(profileEnv, /SDKWORK_DRIVE_FACADE_URL=https:\/\//);
  }
});
