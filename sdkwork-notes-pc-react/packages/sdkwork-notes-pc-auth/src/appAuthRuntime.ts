import { resolveBaseUrl } from '@sdkwork/sdk-common';
import type {
  SdkworkAuthAppearanceConfig,
  SdkworkAuthRuntimeConfig,
  SdkworkIamRuntimeAuthRuntimeLike,
} from '@sdkwork/auth-pc-react';
import {
  createSdkworkAppbasePcAuthRuntime,
  type SdkworkAppbasePcAuthRuntimeComposition,
  type SdkworkAppbasePcAuthRuntimeSdkClient,
} from '@sdkwork/auth-runtime-pc-react';
import {
  applyAppSdkSessionTokens,
  clearAppSdkSessionTokens,
  getNotesGlobalTokenManager,
  readAppSdkSessionTokens,
  resetAppSdkClient,
  resetNotesGlobalTokenManager,
  type AppSdkSessionTokens,
} from '@sdkwork/notes-pc-core';

type IamEnvironment = 'dev' | 'prod' | 'test';
type IamDeploymentMode = 'local' | 'private' | 'saas';

const NOTES_VERIFICATION_POLICY = {
  emailCodeLoginEnabled: false,
  emailRegistrationVerificationRequired: false,
  phoneCodeLoginEnabled: false,
  phoneRegistrationVerificationRequired: false,
} as const;

let notesIamRuntimeComposition: SdkworkAppbasePcAuthRuntimeComposition | null = null;

function readEnvValue(...keys: string[]): string | undefined {
  const meta = import.meta as ImportMeta & {
    env?: Record<string, string | boolean | undefined>;
  };

  for (const key of keys) {
    const value = meta.env?.[key];
    if (typeof value === 'string' && value.trim().length > 0) {
      return value.trim();
    }
  }

  return undefined;
}

function resolveIamEnvironment(): IamEnvironment {
  const value = readEnvValue('VITE_SDKWORK_NOTES_IAM_ENVIRONMENT', 'VITE_SDKWORK_IAM_ENVIRONMENT');
  return value === 'prod' || value === 'production'
    ? 'prod'
    : value === 'test'
      ? 'test'
      : 'dev';
}

function resolveIamDeploymentMode(): IamDeploymentMode {
  const normalized = (readEnvValue(
    'VITE_SDKWORK_DEPLOYMENT_MODE',
    'VITE_SDKWORK_NOTES_IAM_DEPLOYMENT_MODE',
    'VITE_SDKWORK_IAM_DEPLOYMENT_MODE',
  ) ?? 'local').trim().toLowerCase();

  if (normalized === 'saas' || normalized === 'cloud-saas' || normalized === 'cloud') {
    return 'saas';
  }
  if (normalized === 'private' || normalized === 'server-private') {
    return 'private';
  }
  return 'local';
}

function resolveAppbaseAppApiBaseUrl(): string {
  // Authored overrides win; the shared default resolves through
  // @sdkwork/sdk-common resolveBaseUrl (ENVIRONMENT_SPEC.md §6.3): unified
  // SDKWORK_API_BASE_URL candidates matched against the page host, else
  // derived from it (standalone same-origin; cloud api[-<env>].<brand>;
  // pnpm dev local dev-server origin or cloud-gateway dev port).
  return readEnvValue(
    'VITE_SDKWORK_NOTES_PLATFORM_API_GATEWAY_HTTP_URL',
    'VITE_SDKWORK_APPBASE_APP_API_BASE_URL',
    'VITE_SDKWORK_IAM_APP_API_BASE_URL',
  ) ?? resolveBaseUrl().url ?? '';
}

export function resetNotesAuthenticatedSdkClients(): void {
  resetAppSdkClient();
}

export function clearNotesIamRuntimeSession(): void {
  clearAppSdkSessionTokens();
  resetNotesAuthenticatedSdkClients();
}

function getAuthenticatedSdkClients(): SdkworkAppbasePcAuthRuntimeSdkClient[] {
  return [];
}

function createNotesIamRuntime(): SdkworkAppbasePcAuthRuntimeComposition {
  return createSdkworkAppbasePcAuthRuntime({
    app: {
      appId: 'sdkwork-notes-pc-react',
      deploymentMode: resolveIamDeploymentMode(),
      environment: resolveIamEnvironment(),
      platform: 'pc',
    },
    baseUrls: {
      appbaseAppApiBaseUrl: resolveAppbaseAppApiBaseUrl(),
    },
    hooks: {
      onSessionChanged: () => {
        resetNotesAuthenticatedSdkClients();
      },
    },
    sdkClients: getAuthenticatedSdkClients(),
    sessionBridge: {
      clearSession: clearNotesIamRuntimeSession,
      commitSession: (session) => applyAppSdkSessionTokens(session as AppSdkSessionTokens),
      readSession: readAppSdkSessionTokens,
    },
    tokenManager: getNotesGlobalTokenManager(),
  });
}

export function getNotesIamRuntime(): SdkworkIamRuntimeAuthRuntimeLike {
  if (!notesIamRuntimeComposition) {
    notesIamRuntimeComposition = createNotesIamRuntime();
  }

  return notesIamRuntimeComposition.runtime as unknown as SdkworkIamRuntimeAuthRuntimeLike;
}

export function resetNotesIamRuntime(): void {
  notesIamRuntimeComposition = null;
  resetNotesGlobalTokenManager();
}

export function resolveNotesAuthRuntimeConfig(): SdkworkAuthRuntimeConfig {
  return {
    leftRailMode: 'qr-only',
    loginMethods: ['password'],
    oauthLoginEnabled: false,
    oauthProviders: [],
    qrLoginEnabled: true,
    recoveryMethods: [],
    registerMethods: ['email', 'phone'],
    verificationPolicy: NOTES_VERIFICATION_POLICY,
  };
}

export function resolveNotesAuthAppearance(): SdkworkAuthAppearanceConfig {
  return {
    pageClassName: 'sdkwork-notes-auth-page',
    shellClassName: 'sdkwork-notes-auth-card-shell',
  };
}
