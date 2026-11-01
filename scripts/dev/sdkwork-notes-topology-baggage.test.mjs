#!/usr/bin/env node
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

const scanRoots = [
  'crates',
  'scripts',
  'configs',
  'docs',
  'specs',
  'sdks',
  'sdkwork-notes-pc-react/packages',
  'README.md',
  'AGENTS.md',
];

const skipPathFragments = [
  '/target/',
  '/node_modules/',
  '/generated/',
  'sdkwork-notes-topology-baggage.test.mjs',
  'docs/topology-standard.md',
  'verify-notes-topology.test.mjs',
  'docs/archive/',
];

const allowlistPathFragments = [
  'specs/topology.spec.json',
  'etc/topology/',
  'sdkwork-notes-pc-react/.env.standalone.development.example',
  'docs/架构/',
];

const bannedPatterns = [
  { id: 'topology v1 env key', pattern: /SDKWORK_NOTES_TOPOLOGY/u },
  { id: 'client topology v1 env key', pattern: /VITE_NOTES_TOPOLOGY/u },
  { id: 'topology CLI flag', pattern: /--topology\b/u },
  { id: 'retired app api base url env key', pattern: /VITE_APP_API_BASE_URL/u },
  { id: 'retired api base url env key', pattern: /VITE_API_BASE_URL/u },
  { id: 'retired bind address env key', pattern: /SDKWORK_NOTES_BIND_ADDRESS/u },
  { id: 'legacy bridge helper', pattern: /bridgeLegacyServiceEnv/u },
  { id: 'hardcoded drive app sdk port', pattern: /http:\/\/127\.0\.0\.1:18080/u },
  { id: 'hardcoded drive open sdk port', pattern: /http:\/\/127\.0\.0\.1:18082/u },
];

const surfaceUrlKeys = [
  'SDKWORK_NOTES_APPLICATION_PUBLIC_HTTP_URL',
  'VITE_SDKWORK_NOTES_APPLICATION_PUBLIC_HTTP_URL',
  'SDKWORK_NOTES_PLATFORM_API_GATEWAY_HTTP_URL',
  'VITE_SDKWORK_NOTES_PLATFORM_API_GATEWAY_HTTP_URL',
];

function slash(value) {
  return String(value).replaceAll('\\', '/');
}

function shouldSkip(relativePath) {
  const normalized = slash(relativePath);
  return skipPathFragments.some((fragment) => normalized.includes(fragment));
}

function isAllowlisted(relativePath) {
  const normalized = slash(relativePath);
  return allowlistPathFragments.some((fragment) => normalized.includes(fragment));
}

function collectFiles(relativeRoot) {
  const absoluteRoot = path.join(repoRoot, relativeRoot);
  if (!fs.existsSync(absoluteRoot)) {
    return [];
  }
  const stat = fs.statSync(absoluteRoot);
  if (stat.isFile()) {
    return [relativeRoot];
  }
  const files = [];
  for (const entry of fs.readdirSync(absoluteRoot, { withFileTypes: true })) {
    const relativePath = path.join(relativeRoot, entry.name);
    if (shouldSkip(relativePath)) {
      continue;
    }
    if (entry.isDirectory()) {
      files.push(...collectFiles(relativePath));
      continue;
    }
    files.push(relativePath);
  }
  return files;
}

function readText(relativePath) {
  return fs.readFileSync(path.join(repoRoot, relativePath), 'utf8');
}

const files = scanRoots.flatMap((root) => collectFiles(root));

for (const { id, pattern } of bannedPatterns) {
  const hits = [];
  for (const relativePath of files) {
    if (isAllowlisted(relativePath)) {
      continue;
    }
    if (id === 'topology CLI flag' && slash(relativePath).endsWith('-dev.mjs')) {
      continue;
    }
    const text = readText(relativePath);
    if (pattern.test(text)) {
      hits.push(relativePath);
    }
  }
  assert.equal(
    hits.length,
    0,
    `topology baggage (${id}) found in active paths: ${hits.join(', ')}`,
  );
}

assert.ok(fs.existsSync(path.join(repoRoot, 'specs/topology.spec.json')), 'topology spec required');
const spec = JSON.parse(readText('specs/topology.spec.json'));
assert.equal(spec.schemaVersion, 5);
assert.equal(spec.archetype, 'application-http-gateway');
assert.equal(spec.defaults.developmentProfileId, 'standalone.development');

for (const configFile of spec.packaging.cloudConfigFiles) {
  assert.ok(
    fs.existsSync(path.join(repoRoot, 'configs', configFile)),
    `cloud gateway config required: configs/${configFile}`,
  );
}

const profileDir = path.join(repoRoot, 'etc/topology');
const profileFiles = fs.readdirSync(profileDir).filter((name) => name.endsWith('.env'));
assert.ok(profileFiles.length >= 2, 'topology profile env files required');

const packageJson = JSON.parse(readText('package.json'));
assert.match(
  JSON.stringify(packageJson.dependencies ?? {}),
  /"@sdkwork\/app-topology"/u,
  'package.json must depend on @sdkwork/app-topology',
);
assert.match(
  JSON.stringify(packageJson.scripts ?? {}),
  /"dev"\s*:/u,
  'package.json must expose dev',
);

assert.ok(fs.existsSync(path.join(repoRoot, 'scripts/notes-dev.mjs')), 'notes-dev orchestrator required');
assert.ok(
  fs.existsSync(path.join(repoRoot, 'scripts/lib/notes-topology.mjs')),
  'notes topology adapter required',
);
assert.ok(fs.existsSync(path.join(repoRoot, 'docs/topology-standard.md')), 'topology-standard doc required');

for (const profileFile of profileFiles) {
  const profileText = readText(path.join('etc/topology', profileFile));
  for (const key of surfaceUrlKeys) {
    const match = profileText.match(new RegExp(`^${key}=(.+)$`, 'mu'));
    if (!match) {
      continue;
    }
    const value = match[1].trim();
    assert.match(
      value,
      /^https?:\/\/[^/]+(?::\d+)?$/u,
      `${profileFile} ${key} must be an ingress origin without API path prefix; got ${value}`,
    );
  }
}

console.log('[sdkwork-notes-topology-baggage] ok');
