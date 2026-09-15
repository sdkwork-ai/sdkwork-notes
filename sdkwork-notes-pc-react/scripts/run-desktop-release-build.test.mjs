// WORKSPACE-PATH:allow-fixture: fixtures name a foreign checkout root, drive, or home directory to exercise path handling, so the literal is the value under assertion rather than a binding this build resolves
import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';
import { createDesktopReleaseBuildPlan } from './run-desktop-release-build.mjs';

test('bundle plan resolves notes desktop cwd and target environment metadata', () => {
  const plan = createDesktopReleaseBuildPlan({
    cwd: path.join('D:', 'workspace', 'sdkwork-notes-pc-react'),
    targetTriple: 'aarch64-pc-windows-msvc',
    env: {
      USERPROFILE: path.join('C:', 'Users', 'admin'),
      PATH: path.join('C:', 'Windows', 'System32'),
    },
    runtimePlatform: 'win32',
    windowsCargoBinDir: 'C:\\Users\\admin\\.cargo\\bin',
  });

  assert.equal(plan.command, 'pnpm');
  assert.deepEqual(plan.args, ['exec', 'tauri', 'build', '--target', 'aarch64-pc-windows-msvc']);
  assert.equal(
    plan.cwd,
    path.join('D:', 'workspace', 'sdkwork-notes-pc-react', 'packages', 'sdkwork-notes-pc-desktop'),
  );
  assert.equal(plan.env.SDKWORK_DESKTOP_TARGET, 'aarch64-pc-windows-msvc');
  assert.equal(plan.env.SDKWORK_DESKTOP_TARGET_PLATFORM, 'windows');
  assert.equal(plan.env.SDKWORK_DESKTOP_TARGET_ARCH, 'arm64');
  assert.equal(plan.env.SDKWORK_SHARED_SDK_MODE, 'git');
  assert.match(plan.env.PATH, /^C:\\Users\\admin\\\.cargo\\bin;/);
  assert.equal(plan.workspaceRoot, path.join('D:', 'workspace', 'sdkwork-notes-pc-react'));
  assert.equal(plan.phase, 'bundle');
});

test('bundle plan constrains macOS bundles to app output', () => {
  const plan = createDesktopReleaseBuildPlan({
    cwd: '/workspace/sdkwork-notes-pc-react',
    targetTriple: 'x86_64-apple-darwin',
  });

  assert.deepEqual(plan.args, [
    'exec',
    'tauri',
    'build',
    '--target',
    'x86_64-apple-darwin',
    '--bundles',
    'app',
  ]);
});

test('info phase requests tauri info without mutating the selected target triple', () => {
  const plan = createDesktopReleaseBuildPlan({
    phase: 'info',
    cwd: '/workspace/sdkwork-notes-pc-react',
    targetTriple: 'x86_64-unknown-linux-gnu',
  });

  assert.deepEqual(plan.args, ['exec', 'tauri', 'info']);
  assert.equal(plan.target.targetTriple, 'x86_64-unknown-linux-gnu');
  assert.equal(plan.env.SDKWORK_DESKTOP_TARGET_PLATFORM, 'linux');
  assert.equal(plan.env.SDKWORK_DESKTOP_TARGET_ARCH, 'x64');
  assert.equal(plan.env.SDKWORK_SHARED_SDK_MODE, 'git');
  assert.equal(plan.phase, 'info');
});
