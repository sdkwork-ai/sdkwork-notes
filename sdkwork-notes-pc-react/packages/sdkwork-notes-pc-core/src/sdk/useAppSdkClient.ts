import { getApiHostForEnvironment, getBrand } from '@sdkwork/sdk-common';
import { useMemo } from 'react';
import { isBlank, trim } from '@sdkwork/utils';
import { normalizeString } from '@sdkwork/notes-pc-commons';
import type {
  NotesRemoteAppClient,
  NotesRemoteAppClientFactory,
  SdkworkAppConfig,
} from './appSdkPort';
import { resolveSessionIdentityClaims } from './sessionIdentityClaims';
import {
  assertNoForbiddenCredentialEnv,
  resolveSdkworkAccessTokenFromEnv,
} from './appSdkCredentialEnv';

export type AppRuntimeEnv = 'development' | 'staging' | 'production' | 'test';

export interface AppSdkClientConfig extends SdkworkAppConfig {
  env: AppRuntimeEnv;
}

export interface AppSdkClientOverrides extends Partial<SdkworkAppConfig> {
  client?: NotesRemoteAppClient;
  clientFactory?: NotesRemoteAppClientFactory;
}

export interface AppSdkSessionTokens {
  authToken?: string;
  accessToken?: string;
  refreshToken?: string;
}

export type OwnerMode = 'root' | 'tenant' | 'organization';
export type AppRuntimeSourceName = '__SDKWORK_NOTES_ENV__' | 'import.meta.env' | 'process.env';

export interface AppSdkRuntimeContext {
  env: AppRuntimeEnv;
  ownerMode: OwnerMode;
  platform: string;
  sourcePriority: readonly AppRuntimeSourceName[];
}

export interface AppSdkSessionStoreAdapter {
  kind: string;
  read(): AppSdkSessionTokens | null | undefined;
  write(session: AppSdkSessionTokens): void;
  clear(): void;
}

export const APP_SDK_SESSION_STORAGE_KEY = 'sdkwork-notes-auth-session';

const APP_RUNTIME_SOURCE_PRIORITY = [
  '__SDKWORK_NOTES_ENV__',
  'import.meta.env',
  'process.env',
] as const;
const NOTES_ENV_GLOBAL_KEYS = ['__SDKWORK_NOTES_ENV__'];
const AUTH_TOKEN_STORAGE_KEY = 'sdkwork.core.pc-react.auth-token';
const ACCESS_TOKEN_STORAGE_KEY = 'sdkwork.core.pc-react.access-token';
const REFRESH_TOKEN_STORAGE_KEY = 'sdkwork.core.pc-react.refresh-token';
const DEFAULT_TIMEOUT = 30_000;

type EnvSource = Record<string, unknown>;
type RuntimeWindow = Window & {
  __TAURI__?: unknown;
  __TAURI_IPC__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

let appClientCache: NotesRemoteAppClient | null = null;
let appClientConfigCache: AppSdkClientConfig | null = null;
let appSdkSessionStoreAdapter: AppSdkSessionStoreAdapter | null = null;
let appSdkClientFactory: NotesRemoteAppClientFactory | null = null;
let sessionMemorySnapshot: AppSdkSessionTokens | null = null;
const memoryStorage = new Map<string, string>();

function normalizeEnvValue(value: unknown): string | undefined {
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value === 'number' && Number.isFinite(value)) {
    return String(value);
  }
  if (typeof value === 'boolean') {
    return value ? 'true' : 'false';
  }
  return undefined;
}

function normalizeBearerToken(value?: unknown): string {
  const normalized = normalizeString(value);
  if (isBlank(normalized)) {
    return '';
  }

  return trim(normalized.replace(/^Bearer\s+/i, ''));
}

function normalizeUrl(value?: unknown): string {
  return normalizeString(value).replace(/\/+$/g, '');
}

function firstNonEmptyValue(...values: Array<string | undefined>): string | undefined {
  for (const value of values) {
    const normalized = normalizeString(value);
    if (!isBlank(normalized)) {
      return normalized;
    }
  }

  return undefined;
}

function parsePositiveNumber(value?: string, fallback: number = DEFAULT_TIMEOUT): number {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return fallback;
  }

  return parsed;
}

function safeParseJson<T>(value: string | null | undefined): T | null {
  if (!value) {
    return null;
  }

  try {
    return JSON.parse(value) as T;
  } catch {
    return null;
  }
}

function normalizeSessionTokens(session?: AppSdkSessionTokens | null): AppSdkSessionTokens {
  return {
    authToken: normalizeBearerToken(session?.authToken) || undefined,
    accessToken: normalizeBearerToken(session?.accessToken) || undefined,
    refreshToken: normalizeString(session?.refreshToken) || undefined,
  };
}

function hasSessionTokens(session?: AppSdkSessionTokens | null): boolean {
  return Boolean(session?.authToken || session?.accessToken || session?.refreshToken);
}

function readImportMetaEnv(): EnvSource {
  return ((import.meta as { env?: EnvSource }).env ?? {}) as EnvSource;
}

function readProcessEnv(): EnvSource {
  const processEnv = (
    globalThis as typeof globalThis & {
      process?: {
        env?: Record<string, unknown>;
      };
    }
  ).process?.env;

  return processEnv ?? {};
}

function readNamedGlobalEnvSources(globalKeys: string[]): EnvSource {
  return globalKeys.reduce<EnvSource>((accumulator, globalKey) => {
    const source = (globalThis as Record<string, unknown>)[globalKey];
    if (!source || typeof source !== 'object') {
      return accumulator;
    }

    return {
      ...accumulator,
      ...(source as EnvSource),
    };
  }, {});
}

function createNotesEnvSource(): Record<string, string | undefined> {
  const mergedSource = {
    ...readProcessEnv(),
    ...readImportMetaEnv(),
    ...readNamedGlobalEnvSources(NOTES_ENV_GLOBAL_KEYS),
  };

  return Object.fromEntries(
    Object.entries(mergedSource).map(([key, value]) => [key, normalizeEnvValue(value)]),
  );
}

export function resolveAppSdkRuntimeContext(
  overrides: AppSdkClientOverrides = {},
): AppSdkRuntimeContext {
  const envSource = createNotesEnvSource();

  return {
    env: resolveRuntimeEnv(envSource.VITE_APP_ENV, envSource.MODE, envSource.NODE_ENV),
    ownerMode: resolveOwnerMode(envSource),
    platform: normalizeString(overrides.platform) || resolvePlatformId(envSource),
    sourcePriority: [...APP_RUNTIME_SOURCE_PRIORITY],
  };
}

function resolveRuntimeEnv(...values: Array<string | undefined>): AppRuntimeEnv {
  const normalized = firstNonEmptyValue(...values)?.toLowerCase();
  if (normalized === 'production' || normalized === 'prod') {
    return 'production';
  }
  if (normalized === 'test') {
    return 'test';
  }
  if (normalized === 'staging' || normalized === 'stage') {
    return 'staging';
  }

  return 'development';
}

function resolveDefaultBaseUrl(
  env: AppRuntimeEnv,
  envSource: Record<string, string | undefined> = {},
): string {
  // APP_RUNTIME_TOPOLOGY_SPEC section 4.2 / SDK_SPEC section 5.1 step 2:
  // dev:cloud binds the local platform gateway (ip:port). Environment domain
  // families (api-dev., api-test., api.) remain build/deploy defaults.
  const localGateway = normalizeUrl(
    firstNonEmptyValue(
      envSource.VITE_SDKWORK_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL,
      envSource.SDKWORK_LOCAL_PLATFORM_API_GATEWAY_HTTP_URL,
    ),
  );
  // ENVIRONMENT_SPEC.md §6.3: derive the api[-<env>].<brand> family host from
  // the current page brand through @sdkwork/sdk-common helpers instead of a
  // local domain table (dev keeps the local dev:cloud gateway anchor first).
  if (env === 'development' && localGateway) {
    return localGateway;
  }
  const brand = getBrand(
    typeof window !== 'undefined' && window.location?.hostname
      ? window.location.hostname
      : 'sdkwork.com',
  );
  return `https://${getApiHostForEnvironment(env === 'development' ? 'dev' : env, brand)}`;
}

function resolveOwnerMode(envSource: Record<string, string | undefined>): OwnerMode {
  const explicitOwnerMode = firstNonEmptyValue(
    envSource.VITE_APP_OWNER_MODE,
    envSource.VITE_OWNER_MODE,
    envSource.SDKWORK_OWNER_MODE,
  )?.toLowerCase();

  if (explicitOwnerMode === 'organization' || explicitOwnerMode === 'org') {
    return 'organization';
  }
  if (explicitOwnerMode === 'tenant') {
    return 'tenant';
  }

  const hasTenantSignals = Boolean(
    firstNonEmptyValue(
      envSource.VITE_TENANT_API_BASE_URL,
      envSource.VITE_APP_TENANT_API_BASE_URL,
    ),
  );
  if (hasTenantSignals) {
    return 'tenant';
  }

  const hasOrganizationSignals = Boolean(
    firstNonEmptyValue(
      envSource.VITE_ORGANIZATION_API_BASE_URL,
      envSource.VITE_APP_ORGANIZATION_API_BASE_URL,
    ),
  );
  if (hasOrganizationSignals) {
    return 'organization';
  }

  return 'root';
}

function pickOwnerScopedValue(
  ownerMode: OwnerMode,
  values: {
    default?: string;
    root?: string;
    tenant?: string;
    organization?: string;
  },
): string | undefined {
  if (ownerMode === 'tenant') {
    return values.tenant ?? values.default;
  }
  if (ownerMode === 'organization') {
    return values.organization ?? values.default;
  }
  return values.root ?? values.default;
}

function detectTauriRuntime(): boolean {
  if (typeof window === 'undefined') {
    return false;
  }

  const runtimeWindow = window as RuntimeWindow;
  return Boolean(runtimeWindow.__TAURI__ || runtimeWindow.__TAURI_IPC__ || runtimeWindow.__TAURI_INTERNALS__);
}

function resolvePlatformId(envSource: Record<string, string | undefined>): string {
  if (detectTauriRuntime()) {
    return 'desktop';
  }

  return (
    firstNonEmptyValue(
      envSource.VITE_PLATFORM,
      envSource.VITE_APP_PLATFORM,
      envSource.SDKWORK_PLATFORM,
    )
    || 'web'
  );
}

function resolveConfiguredAccessToken(
  overrides: AppSdkClientOverrides = {},
  config: AppSdkClientConfig | null = appClientConfigCache,
): string {
  if (overrides.accessToken !== undefined) {
    return normalizeBearerToken(overrides.accessToken);
  }
  if (config?.accessToken) {
    return normalizeBearerToken(config.accessToken);
  }
  return normalizeBearerToken(createAppSdkClientConfig().accessToken);
}

function createMemoryStorage(): Storage {
  return {
    get length() {
      return memoryStorage.size;
    },
    clear() {
      memoryStorage.clear();
    },
    getItem(key) {
      return memoryStorage.get(key) ?? null;
    },
    key(index) {
      return Array.from(memoryStorage.keys())[index] ?? null;
    },
    removeItem(key) {
      memoryStorage.delete(key);
    },
    setItem(key, value) {
      memoryStorage.set(key, value);
    },
  };
}

function resolveSessionStorage(): Storage {
  if (typeof globalThis.localStorage !== 'undefined') {
    migrateLegacySessionStorage();
    return globalThis.localStorage;
  }

  return createMemoryStorage();
}

function migrateLegacySessionStorage(): void {
  if (typeof globalThis.sessionStorage === 'undefined' || typeof globalThis.localStorage === 'undefined') {
    return;
  }
  const legacySession = globalThis.sessionStorage.getItem(APP_SDK_SESSION_STORAGE_KEY);
  if (legacySession && !globalThis.localStorage.getItem(APP_SDK_SESSION_STORAGE_KEY)) {
    globalThis.localStorage.setItem(APP_SDK_SESSION_STORAGE_KEY, legacySession);
  }
  if (legacySession) {
    globalThis.sessionStorage.removeItem(APP_SDK_SESSION_STORAGE_KEY);
  }
}

function resolveLocalStorage(): Storage | null {
  if (typeof globalThis.localStorage !== 'undefined') {
    return globalThis.localStorage;
  }

  return null;
}

function readStorageValue(storage: Storage | null | undefined, key: string): string | undefined {
  if (!storage) {
    return undefined;
  }

  try {
    return storage.getItem(key) ?? undefined;
  } catch {
    return undefined;
  }
}

function writeStorageValue(storage: Storage | null | undefined, key: string, value?: string): void {
  if (!storage) {
    return;
  }

  try {
    const normalized = trim(value ?? '');
    if (!isBlank(normalized)) {
      storage.setItem(key, normalized);
      return;
    }

    storage.removeItem(key);
  } catch {
    // ignore storage write failures
  }
}

function clearStorageKeys(storage: Storage | null | undefined, keys: string[]): void {
  for (const key of keys) {
    writeStorageValue(storage, key, undefined);
  }
}

function readStorageSessionSnapshot(storage: Storage | null | undefined): AppSdkSessionTokens {
  const session = safeParseJson<AppSdkSessionTokens>(
    readStorageValue(storage, APP_SDK_SESSION_STORAGE_KEY),
  );
  const authToken = normalizeBearerToken(
    session?.authToken || readStorageValue(storage, AUTH_TOKEN_STORAGE_KEY),
  );
  const accessToken = normalizeBearerToken(
    session?.accessToken || readStorageValue(storage, ACCESS_TOKEN_STORAGE_KEY),
  );
  const refreshToken = normalizeString(
    session?.refreshToken || readStorageValue(storage, REFRESH_TOKEN_STORAGE_KEY),
  );

  return {
    authToken: authToken || undefined,
    accessToken: accessToken || undefined,
    refreshToken: refreshToken || undefined,
  };
}

function writeStorageSessionSnapshot(
  storage: Storage | null | undefined,
  session: AppSdkSessionTokens,
): void {
  if (!storage) {
    return;
  }

  const normalizedSession = normalizeSessionTokens(session);
  if (!hasSessionTokens(normalizedSession)) {
    clearStorageKeys(storage, [
      APP_SDK_SESSION_STORAGE_KEY,
      AUTH_TOKEN_STORAGE_KEY,
      ACCESS_TOKEN_STORAGE_KEY,
      REFRESH_TOKEN_STORAGE_KEY,
    ]);
    return;
  }

  writeStorageValue(storage, APP_SDK_SESSION_STORAGE_KEY, JSON.stringify(normalizedSession));
  clearStorageKeys(storage, [
    AUTH_TOKEN_STORAGE_KEY,
    ACCESS_TOKEN_STORAGE_KEY,
    REFRESH_TOKEN_STORAGE_KEY,
  ]);
}

function clearLegacyLocalStorageSession(): void {
  clearStorageKeys(resolveLocalStorage(), [
    APP_SDK_SESSION_STORAGE_KEY,
    AUTH_TOKEN_STORAGE_KEY,
    ACCESS_TOKEN_STORAGE_KEY,
    REFRESH_TOKEN_STORAGE_KEY,
  ]);
}

const defaultAppSdkSessionStoreAdapter: AppSdkSessionStoreAdapter = {
  kind: 'browser-local',
  read() {
    const sessionStorageSnapshot = normalizeSessionTokens(
      readStorageSessionSnapshot(resolveSessionStorage()),
    );
    if (hasSessionTokens(sessionStorageSnapshot)) {
      sessionMemorySnapshot = sessionStorageSnapshot;
      return sessionStorageSnapshot;
    }

    const legacyLocalStorageSnapshot = normalizeSessionTokens(
      readStorageSessionSnapshot(resolveLocalStorage()),
    );
    if (hasSessionTokens(legacyLocalStorageSnapshot)) {
      sessionMemorySnapshot = legacyLocalStorageSnapshot;
      writeStorageSessionSnapshot(resolveSessionStorage(), legacyLocalStorageSnapshot);
      return legacyLocalStorageSnapshot;
    }

    return sessionMemorySnapshot;
  },
  write(session) {
    const normalizedSession = normalizeSessionTokens(session);
    sessionMemorySnapshot = hasSessionTokens(normalizedSession) ? normalizedSession : null;
    writeStorageSessionSnapshot(resolveSessionStorage(), normalizedSession);
  },
  clear() {
    sessionMemorySnapshot = null;
    writeStorageSessionSnapshot(resolveSessionStorage(), {});
  },
};

function resolveAppSdkSessionStoreAdapter(): AppSdkSessionStoreAdapter {
  return appSdkSessionStoreAdapter ?? defaultAppSdkSessionStoreAdapter;
}

export function configureAppSdkSessionStoreAdapter(
  adapter: AppSdkSessionStoreAdapter | null,
): void {
  appSdkSessionStoreAdapter = adapter;
  sessionMemorySnapshot = adapter
    ? (() => {
        const snapshot = normalizeSessionTokens(adapter.read() ?? undefined);
        return hasSessionTokens(snapshot) ? snapshot : null;
      })()
    : null;
}

function readPersistedSession(): AppSdkSessionTokens {
  const session = normalizeSessionTokens(
    resolveAppSdkSessionStoreAdapter().read() ?? sessionMemorySnapshot,
  );
  sessionMemorySnapshot = hasSessionTokens(session) ? session : null;
  return session;
}

function readRuntimeSession(): AppSdkSessionTokens {
  const storedSession = readPersistedSession();
  const configuredAccessToken = resolveConfiguredAccessToken();

  return {
    authToken: storedSession.authToken,
    accessToken: storedSession.accessToken || configuredAccessToken || undefined,
    refreshToken: storedSession.refreshToken,
  };
}

function writeSessionSnapshot(session: AppSdkSessionTokens): void {
  const snapshot = normalizeSessionTokens(session);
  if (!hasSessionTokens(snapshot)) {
    resolveAppSdkSessionStoreAdapter().clear();
    return;
  }

  resolveAppSdkSessionStoreAdapter().write(snapshot);
}

function applySessionTokensToClient(
  client: NotesRemoteAppClient,
  session: AppSdkSessionTokens,
  fallbackAccessToken: string,
): void {
  client.setAuthToken?.(normalizeBearerToken(session.authToken));
  client.setAccessToken?.(normalizeBearerToken(session.accessToken || fallbackAccessToken));
}

function resolveEffectiveSession(
  overrides: AppSdkClientOverrides = {},
  fallbackAccessToken: string,
): AppSdkSessionTokens {
  const session = readRuntimeSession();

  return {
    authToken:
      overrides.authToken !== undefined
        ? normalizeBearerToken(overrides.authToken)
        : normalizeBearerToken(session.authToken),
    accessToken:
      overrides.accessToken !== undefined
        ? normalizeBearerToken(overrides.accessToken)
        : normalizeBearerToken(session.accessToken || fallbackAccessToken),
    refreshToken: normalizeString(session.refreshToken),
  };
}

function resolveInjectedAppSdkClient(
  overrides: AppSdkClientOverrides,
  config: AppSdkClientConfig,
): NotesRemoteAppClient {
  if (overrides.client) {
    return overrides.client;
  }

  const factory = overrides.clientFactory ?? appSdkClientFactory;
  if (!factory) {
    throw new Error(
      'SDKWork Notes product app client is not configured. Inject a generated product app SDK client, dependency SDK facade, or approved composed runtime client before using remote Notes services.',
    );
  }

  return factory(config);
}

function createScopedClient(overrides: AppSdkClientOverrides = {}): NotesRemoteAppClient {
  const config = createAppSdkClientConfig(overrides);
  const client = resolveInjectedAppSdkClient(overrides, config);
  applySessionTokensToClient(client, resolveEffectiveSession(overrides, config.accessToken || ''), config.accessToken || '');
  return client;
}

export function createAppSdkClientConfig(
  overrides: AppSdkClientOverrides = {},
): AppSdkClientConfig {
  const envSource = createNotesEnvSource();
  assertNoForbiddenCredentialEnv(envSource);
  const runtimeContext = resolveAppSdkRuntimeContext(overrides);
  const env = runtimeContext.env;
  const ownerMode = runtimeContext.ownerMode;
  const defaultBaseUrl = normalizeUrl(
    firstNonEmptyValue(
      envSource.VITE_SDKWORK_NOTES_APPLICATION_PUBLIC_HTTP_URL,
      envSource.VITE_APP_BASE_URL,
      envSource.SDKWORK_API_BASE_URL,
      resolveDefaultBaseUrl(env, envSource),
    ),
  );
  const baseUrl = normalizeUrl(
    pickOwnerScopedValue(ownerMode, {
      default: defaultBaseUrl,
      root: firstNonEmptyValue(envSource.VITE_ROOT_API_BASE_URL, envSource.VITE_APP_ROOT_API_BASE_URL, defaultBaseUrl),
      tenant: firstNonEmptyValue(envSource.VITE_TENANT_API_BASE_URL, envSource.VITE_APP_TENANT_API_BASE_URL, defaultBaseUrl),
      organization: firstNonEmptyValue(
        envSource.VITE_ORGANIZATION_API_BASE_URL,
        envSource.VITE_APP_ORGANIZATION_API_BASE_URL,
        defaultBaseUrl,
      ),
    }) || resolveDefaultBaseUrl(env, envSource),
  );
  const accessToken = normalizeBearerToken(
    resolveSdkworkAccessTokenFromEnv(envSource) || undefined,
  );
  const authToken = normalizeBearerToken(overrides.authToken);
  const overrideApiKey = normalizeString(overrides.apiKey);
  const resolvedApiKey = overrideApiKey || undefined;
  const resolvedAccessToken = normalizeBearerToken(overrides.accessToken) || accessToken || undefined;
  const resolvedAuthMode =
    overrides.authMode
    || (resolvedApiKey && !resolvedAccessToken && !authToken ? 'apikey' : 'dual-token');

  const persistedSession = readPersistedSession();
  const identityClaims = resolveSessionIdentityClaims({
    accessToken: resolvedAccessToken || persistedSession.accessToken,
    authToken: authToken || persistedSession.authToken,
  });

  return {
    env,
    baseUrl: normalizeUrl(overrides.baseUrl) || baseUrl,
    timeout: overrides.timeout ?? parsePositiveNumber(firstNonEmptyValue(envSource.VITE_TIMEOUT, envSource.SDKWORK_TIMEOUT)),
    apiKey: resolvedApiKey,
    authToken: authToken || undefined,
    accessToken: resolvedAccessToken,
    tenantId: normalizeString(overrides.tenantId) || identityClaims.tenantId || undefined,
    organizationId:
      normalizeString(overrides.organizationId)
      || identityClaims.organizationId
      || undefined,
    platform: runtimeContext.platform,
    tokenManager: overrides.tokenManager,
    authMode: resolvedAuthMode,
    headers: {
      ...(overrides.headers ?? {}),
    },
  };
}

export function initAppSdkClient(
  overrides: AppSdkClientOverrides = {},
): NotesRemoteAppClient {
  const config = createAppSdkClientConfig(overrides);
  const client = resolveInjectedAppSdkClient(overrides, config);
  applySessionTokensToClient(client, resolveEffectiveSession(overrides, config.accessToken || ''), config.accessToken || '');
  appClientCache = client;
  appClientConfigCache = config;
  return client;
}

export function getAppSdkClient(): NotesRemoteAppClient {
  return appClientCache ?? initAppSdkClient();
}

export function getAppSdkClientConfig(): AppSdkClientConfig | null {
  return appClientConfigCache;
}

export function resolveAppSdkAccessToken(): string {
  const sessionAccessToken = normalizeBearerToken(readPersistedSession().accessToken);
  if (sessionAccessToken) {
    return sessionAccessToken;
  }

  const configuredAccessToken = normalizeBearerToken(appClientConfigCache?.accessToken);
  if (configuredAccessToken) {
    return configuredAccessToken;
  }

  return resolveConfiguredAccessToken();
}

export function resetAppSdkClient(): void {
  appClientCache = null;
  appClientConfigCache = null;
  sessionMemorySnapshot = null;
}

export function configureAppSdkClientFactory(
  factory: NotesRemoteAppClientFactory | null,
): void {
  appSdkClientFactory = factory;
  appClientCache = null;
  appClientConfigCache = null;
}

export function applyAppSdkSessionTokens(tokens: AppSdkSessionTokens): void {
  const client = getAppSdkClient();
  applySessionTokensToClient(
    client,
    {
      ...readRuntimeSession(),
      ...tokens,
      accessToken: tokens.accessToken ?? resolveAppSdkAccessToken(),
    },
    resolveConfiguredAccessToken({}, appClientConfigCache),
  );
}

export function readAppSdkSessionTokens(): AppSdkSessionTokens {
  const session = readRuntimeSession();
  const accessToken = session.accessToken || resolveAppSdkAccessToken();

  return {
    authToken: session.authToken,
    accessToken: accessToken || undefined,
    refreshToken: session.refreshToken,
  };
}

export function persistAppSdkSessionTokens(tokens: AppSdkSessionTokens): void {
  const currentSession = readRuntimeSession();
  const fallbackAccessToken = resolveConfiguredAccessToken();
  const nextSession: AppSdkSessionTokens = {
    authToken:
      tokens.authToken !== undefined
        ? normalizeBearerToken(tokens.authToken) || undefined
        : currentSession.authToken,
    accessToken:
      tokens.accessToken !== undefined
        ? normalizeBearerToken(tokens.accessToken) || undefined
        : currentSession.accessToken || fallbackAccessToken || undefined,
    refreshToken:
      tokens.refreshToken !== undefined
        ? normalizeString(tokens.refreshToken) || undefined
        : currentSession.refreshToken,
  };
  const persistedAccessToken =
    normalizeBearerToken(nextSession.accessToken) === normalizeBearerToken(fallbackAccessToken)
      ? undefined
      : normalizeBearerToken(nextSession.accessToken) || undefined;

  writeSessionSnapshot({
    authToken: nextSession.authToken,
    accessToken: persistedAccessToken,
    refreshToken: nextSession.refreshToken,
  });

  if (appClientCache) {
    applySessionTokensToClient(appClientCache, nextSession, fallbackAccessToken);
  }
}

export function clearAppSdkSessionTokens(): void {
  resolveAppSdkSessionStoreAdapter().clear();

  if (appClientCache) {
    applySessionTokensToClient(
      appClientCache,
      {
        authToken: undefined,
        accessToken: resolveConfiguredAccessToken(),
      },
      resolveConfiguredAccessToken({}, appClientConfigCache),
    );
  }
}

export function getAppSdkClientWithSession(
  overrides: AppSdkClientOverrides = {},
): NotesRemoteAppClient {
  if (Object.keys(overrides).length > 0) {
    return createScopedClient(overrides);
  }

  const client = getAppSdkClient();
  const config = appClientConfigCache ?? createAppSdkClientConfig();
  applySessionTokensToClient(client, readRuntimeSession(), config.accessToken || '');
  return client;
}

export function useAppSdkClient(
  overrides: AppSdkClientOverrides = {},
): NotesRemoteAppClient {
  const key = JSON.stringify(overrides || {});
  return useMemo(() => getAppSdkClientWithSession(overrides), [key]);
}
