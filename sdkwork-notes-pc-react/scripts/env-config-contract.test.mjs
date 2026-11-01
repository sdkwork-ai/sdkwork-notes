import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const workspaceRoot = process.cwd();

function read(relativePath) {
  return fs.readFileSync(path.join(workspaceRoot, relativePath), 'utf8');
}

test('workspace ships explicit development, test, production, and example env files', () => {
  for (const relativePath of [
    '.env.example',
    '.env.standalone.development',
    '.env.standalone.test',
    '.env.standalone.production',
    '.env.standalone.development.example',
  ]) {
    assert.equal(fs.existsSync(path.join(workspaceRoot, relativePath)), true, `${relativePath} is required`);
  }
});

test('workspace env defaults align topology surface urls with sdkwork-notes profiles', () => {
  const envExample = read('.env.example');
  const envDevelopment = read('.env.standalone.development');
  const envTest = read('.env.standalone.test');
  const envProduction = read('.env.standalone.production');

  for (const envText of [envExample, envDevelopment, envTest, envProduction]) {
    assert.match(envText, /VITE_SDKWORK_NOTES_APPLICATION_PUBLIC_HTTP_URL=/);
    assert.match(envText, /VITE_SDKWORK_NOTES_PLATFORM_API_GATEWAY_HTTP_URL=/);
    assert.match(envText, /VITE_SDKWORK_APPBASE_APP_API_BASE_URL=/);
    assert.doesNotMatch(envText, /VITE_APP_API_BASE_URL=/);
    assert.doesNotMatch(envText, /VITE_API_BASE_URL=/);
  }

  assert.match(envExample, /VITE_SDKWORK_NOTES_APPLICATION_PUBLIC_HTTP_URL=https:\/\/api-dev\.sdkwork\.com/);
  assert.match(envExample, /SDKWORK_ACCESS_TOKEN=$/m);
  assert.match(envExample, /VITE_APP_OWNER_MODE=tenant/);

  assert.match(envDevelopment, /VITE_APP_ENV=development/);
  assert.match(envDevelopment, /VITE_SDKWORK_NOTES_PLATFORM_API_GATEWAY_HTTP_URL=https:\/\/api-dev\.sdkwork\.com/);

  assert.match(envTest, /VITE_APP_ENV=test/);
  assert.match(envTest, /VITE_SDKWORK_NOTES_APPLICATION_PUBLIC_HTTP_URL=https:\/\/api-test\.sdkwork\.com/);

  assert.match(envProduction, /VITE_APP_ENV=production/);
  assert.match(envProduction, /VITE_SDKWORK_NOTES_APPLICATION_PUBLIC_HTTP_URL=https:\/\/notes\.sdkwork\.com/);
  assert.match(envProduction, /VITE_SDKWORK_NOTES_PLATFORM_API_GATEWAY_HTTP_URL=https:\/\/api\.sdkwork\.com/);
});

test('web and desktop vite configs load workspace env files and allow overriding app mode', () => {
  const webViteConfig = read('vite.config.ts');
  const desktopViteConfig = read('packages/sdkwork-notes-pc-desktop/vite.config.ts');

  assert.match(webViteConfig, /loadEnv\(.*workspaceRootDir.*''\)/);
  assert.match(webViteConfig, /envDir:\s*workspaceRootDir/);
  assert.match(webViteConfig, /SDKWORK_NOTES_APP_MODE/);

  assert.match(desktopViteConfig, /loadEnv\(.*workspaceRootDir.*''\)/);
  assert.match(desktopViteConfig, /envDir:\s*workspaceRootDir/);
  assert.match(desktopViteConfig, /SDKWORK_NOTES_APP_MODE/);
  assert.match(desktopViteConfig, /VITE_APP_PLATFORM/);
  assert.match(webViteConfig, /vite-private-env/);
  assert.match(desktopViteConfig, /vite-private-env/);
  assert.doesNotMatch(webViteConfig, /@sdkwork\/core-pc-react/);
  assert.doesNotMatch(desktopViteConfig, /@sdkwork\/core-pc-react/);
});
