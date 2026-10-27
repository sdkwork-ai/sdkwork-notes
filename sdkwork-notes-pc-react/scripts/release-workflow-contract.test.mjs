import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import test from 'node:test';

const workspaceRoot = process.cwd();
const workflowPath = path.resolve(
  workspaceRoot,
  '../.github/workflows/package.yml',
);
const legacyWorkflowPath = path.resolve(
  workspaceRoot,
  '../.github/workflows/sdkwork-notes-desktop-release.yml',
);
const workflowConfigPath = path.resolve(workspaceRoot, '../agent.workflow.json');

test('release workflow delegates desktop packaging to the SDKWork reusable workflow', () => {
  const workflowSource = fs.readFileSync(workflowPath, 'utf8');
  const workflowConfig = JSON.parse(fs.readFileSync(workflowConfigPath, 'utf8'));
  const targetIds = workflowConfig.targets.map((target) => target.id);

  assert.equal(fs.existsSync(legacyWorkflowPath), false);
  assert.match(workflowSource, /name:\s+Package Application/i);
  assert.match(
    workflowSource,
    /uses:\s+sdkwork-ai\/sdkwork-github-workflow\/\.github\/workflows\/sdkwork-package\.yml@b0829529b9277a3da32b90c2d36ff34ff09fa832/,
  );
  assert.match(workflowSource, /config_path:\s+agent\.workflow\.json/);
  assert.match(
    workflowSource,
    /package_version:\s+\$\{\{\s*github\.event\.inputs\.package_version\s+\|\|\s+''\s*\}\}/,
  );

  assert.equal(workflowConfig.app.id, 'sdkwork-notes-pc-react');
  assert.equal(workflowConfig.app.sourcePath, 'sdkwork-notes-pc-react');
  assert.equal(workflowConfig.release.changelog.source, 'auto');
  assert.deepEqual(targetIds, [
    'windows-x64-standalone-desktop-msi',
    'windows-x64-standalone-desktop-exe',
    'windows-arm64-standalone-desktop-msi',
    'windows-arm64-standalone-desktop-exe',
    'linux-debian-x64-standalone-desktop-deb',
    'linux-rhel-x64-standalone-desktop-rpm',
    'linux-x64-standalone-desktop-appimage',
    'linux-debian-arm64-standalone-desktop-deb',
    'linux-rhel-arm64-standalone-desktop-rpm',
    'linux-arm64-standalone-desktop-appimage',
    'macos-x64-standalone-desktop-tar-gz',
    'macos-arm64-standalone-desktop-tar-gz',
  ]);
});
