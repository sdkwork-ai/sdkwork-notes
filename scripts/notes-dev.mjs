#!/usr/bin/env node

import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import {
  DEFAULT_DEV_PROFILE_ID,
  IAM_APPLICATION_BOOTSTRAP_ENV,
  listHealthSurfaces,
  listOrchestrationProcesses,
  loadProfile,
  mergeRuntimeEnv,
  PC_REACT_ROOT,
  REPO_ROOT,
  resolveDevProfileId,
  resolveIamDevEnv,
  resolveSurfaceHttpUrl,
  waitForHttpHealthy,
} from './lib/notes-topology.mjs';
import { mergeRepoDevBootstrapAccessTokenEnv } from '../../sdkwork-iam/scripts/dev/create-dev-bootstrap-access-token-env.mjs';

const HEALTH_PATH = '/healthz';
const HEALTH_TIMEOUT_MS = 2000;
const STARTUP_WAIT_MS = 500;
const MAX_STARTUP_ATTEMPTS = 60;

function cargoCommand() {
  return process.platform === 'win32' ? 'cargo.exe' : 'cargo';
}

function pnpmCommand() {
  return process.platform === 'win32' ? 'pnpm.cmd' : 'pnpm';
}

function parseArgs(argv) {
  const settings = {
    deploymentProfile: 'standalone',
    runtimeTarget: 'browser',
    dryRun: false,
    help: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === '--help' || arg === '-h') {
      settings.help = true;
      continue;
    }
    if (arg === '--deployment-profile') {
      settings.deploymentProfile = argv[index + 1] ?? settings.deploymentProfile;
      index += 1;
      continue;
    }
    if (arg === '--service-layout') {
      throw new Error('--service-layout is retired; topology v5 uses <deploymentProfile>.<environment>');
    }
    if (arg === '--runtime-target') {
      settings.runtimeTarget = argv[index + 1] ?? settings.runtimeTarget;
      index += 1;
      continue;
    }
    if (arg === '--hosting') {
      throw new Error('--hosting is retired; use --deployment-profile standalone|cloud');
    }
    if (arg === '--target') {
      throw new Error('--target is retired; use --runtime-target browser|desktop');
    }
    if (arg === '--topology') {
      throw new Error('--topology is retired; use --deployment-profile standalone|cloud');
    }
    if (arg === '--dry-run') {
      settings.dryRun = true;
    }
  }

  return settings;
}

function printHelp() {
  console.log(`Usage: node scripts/notes-dev.mjs [options]

Topology-aware Notes dev entry. Loads etc/topology profile env via @sdkwork/app-topology.

Options:
  --deployment-profile <standalone|cloud>           Default: standalone
  --runtime-target <browser|desktop>                Default: browser
  --dry-run                                         Print plan without executing
  --help, -h
`);
}

function spawnProcessEntry(entry) {
  return spawn(entry.command, entry.args, {
    cwd: entry.cwd ?? REPO_ROOT,
    env: entry.env,
    stdio: 'inherit',
    shell: false,
    windowsHide: true,
  });
}

function terminateProcessTree(child) {
  if (!child?.pid) {
    return;
  }
  if (process.platform === 'win32') {
    spawnSync('taskkill.exe', ['/PID', String(child.pid), '/T', '/F'], {
      stdio: 'ignore',
      windowsHide: true,
    });
    return;
  }
  child.kill();
}

function ensureNotesDataDir() {
  const dataDir = path.join(REPO_ROOT, '.sdkwork', 'notes');
  if (!fs.existsSync(dataDir)) {
    fs.mkdirSync(dataDir, { recursive: true });
  }
}

function createNotesApiServerProcess(env) {
  ensureNotesDataDir();
  return {
    label: 'sdkwork-api-notes-standalone-gateway',
    command: cargoCommand(),
    args: ['run', '-p', 'sdkwork-api-notes-standalone-gateway'],
    cwd: REPO_ROOT,
    env,
  };
}

function buildProcessesFromOrchestration(profileId, env, runtimeTarget) {
  const processes = [];

  for (const processDef of listOrchestrationProcesses(profileId)) {
    if (processDef.id === 'platform.api-gateway') {
      continue;
    }

    if (processDef.id === 'application.public-ingress') {
      processes.push(createNotesApiServerProcess(env));
      continue;
    }

    if (processDef.id === 'pc-renderer') {
      processes.push(createPcRendererProcess(env, runtimeTarget));
    }
  }

  return processes;
}

function createPcRendererProcess(env, runtimeTarget) {
  const script = runtimeTarget === 'desktop' ? 'dev:desktop' : 'dev';
  return {
    label: `sdkwork-notes-pc-react:${script}`,
    command: pnpmCommand(),
    args: ['run', script],
    cwd: PC_REACT_ROOT,
    env,
  };
}

async function waitForSurfaceHealth(profileId, env) {
  const surfaces = listHealthSurfaces(profileId);
  for (const surfaceId of surfaces) {
    const url = resolveSurfaceHttpUrl(env, surfaceId);
    if (!url) {
      continue;
    }
    const healthUrl = `${url.replace(/\/+$/u, '')}${HEALTH_PATH}`;
    let ready = false;
    for (let attempt = 0; attempt < MAX_STARTUP_ATTEMPTS; attempt += 1) {
      ready = await waitForHttpHealthy(healthUrl, HEALTH_TIMEOUT_MS);
      if (ready) {
        console.log(`[sdkwork-notes] healthy ${surfaceId} (${healthUrl})`);
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, STARTUP_WAIT_MS));
    }
    if (!ready) {
      throw new Error(`timed out waiting for ${surfaceId} health at ${healthUrl}`);
    }
  }
}

async function main() {
  const settings = parseArgs(process.argv.slice(2));
  if (settings.help) {
    printHelp();
    process.exit(0);
  }

  const profileId = resolveDevProfileId(settings.deploymentProfile)
    || DEFAULT_DEV_PROFILE_ID;
  const profileEnv = loadProfile(profileId);
  const runtimeEnv = mergeRepoDevBootstrapAccessTokenEnv({
    repoRoot: REPO_ROOT,
    manifestPath: 'sdkwork-notes-pc-react/agent.app.config.json',
    env: mergeRuntimeEnv(
      process.env,
      profileEnv,
      resolveIamDevEnv(process.env),
      {
        SDKWORK_NOTES_PROFILE_ID: profileId,
        SDKWORK_NOTES_DEV_MODE: '1',
        ...IAM_APPLICATION_BOOTSTRAP_ENV,
      },
    ),
  });

  const processes = buildProcessesFromOrchestration(profileId, runtimeEnv, settings.runtimeTarget);

  if (settings.dryRun) {
    console.log(`[sdkwork-notes] profile=${profileId} runtimeTarget=${settings.runtimeTarget}`);
    for (const entry of processes) {
      console.log(`[${entry.label}] ${entry.command} ${entry.args.join(' ')}`);
    }
    process.exit(0);
  }

  const children = [];
  let shuttingDown = false;

  function shutdown(exceptChild) {
    if (shuttingDown) {
      return;
    }
    shuttingDown = true;
    for (const child of children) {
      if (child !== exceptChild && child.exitCode == null && child.signalCode == null) {
        terminateProcessTree(child);
      }
    }
  }

  function attachProcessLifecycle(entry, child) {
    child.on('error', (error) => {
      process.stderr.write(
        `[${entry.label}] ${error instanceof Error ? error.message : String(error)}\n`,
      );
      shutdown(child);
      process.exitCode = 1;
    });
    child.on('exit', (code, signal) => {
      if (shuttingDown) {
        return;
      }
      shutdown(child);
      if (code && code !== 0) {
        process.stderr.write(`[${entry.label}] exited with code ${code}\n`);
        process.exitCode = code;
        return;
      }
      if (signal) {
        process.stderr.write(`[${entry.label}] exited with signal ${signal}\n`);
        process.exitCode = 1;
      }
    });
  }

  for (const entry of processes) {
    const child = spawnProcessEntry(entry);
    children.push(child);
    attachProcessLifecycle(entry, child);
  }

  try {
    await waitForSurfaceHealth(profileId, runtimeEnv);
  } catch (error) {
    shutdown();
    throw error;
  }

  console.log(`[sdkwork-notes] dev stack ready (profile=${profileId}, runtimeTarget=${settings.runtimeTarget})`);
  const stop = () => shutdown();
  process.once('SIGINT', stop);
  process.once('SIGTERM', stop);
}

main().catch((error) => {
  console.error(`[sdkwork-notes] ${error instanceof Error ? error.message : String(error)}`);
  process.exit(1);
});
