import { api } from './client';
import type { Agent, BrowserInstance, LoginResponse, TokenRecord } from './types';

export function login(email: string, password: string) {
  return api<LoginResponse>('/api/v1/auth/login', {
    method: 'POST',
    body: JSON.stringify({ email, password }),
  });
}

export async function listBrowsers(tenantId: string) {
  const response = await api<{ data: BrowserInstance[] }>(`/api/v1/tenants/${tenantId}/browser-instances`);
  return response.data;
}

export async function getBrowser(tenantId: string, browserId: string) {
  const response = await api<{ data: BrowserInstance }>(`/api/v1/tenants/${tenantId}/browser-instances/${browserId}`);
  return response.data;
}

export async function resetBrowser(tenantId: string, browserId: string) {
  const response = await api<{ data: BrowserInstance }>(`/api/v1/tenants/${tenantId}/browser-instances/${browserId}/reset`, {
    method: 'POST',
  });
  return response.data;
}

// --- CDP Tokens ---

export async function listCdpTokens(tenantId: string) {
  const response = await api<{ data: TokenRecord[] }>(`/api/v1/tenants/${tenantId}/tokens/cdp`);
  return response.data;
}

export async function createCdpToken(tenantId: string, name?: string) {
  return api<{ data: TokenRecord; token: string }>(`/api/v1/tenants/${tenantId}/tokens/cdp`, {
    method: 'POST',
    body: JSON.stringify({ name: name ?? 'CDP access token' }),
  });
}

export async function revokeCdpToken(tenantId: string, tokenId: string) {
  return api<void>(`/api/v1/tenants/${tenantId}/tokens/cdp/${tokenId}/revoke`, {
    method: 'POST',
  });
}

export async function rotateCdpToken(tenantId: string, tokenId: string) {
  return api<{ data: TokenRecord; token: string }>(`/api/v1/tenants/${tenantId}/tokens/cdp/${tokenId}/rotate`, {
    method: 'POST',
  });
}

// --- Agents ---

export async function listAgents(tenantId: string) {
  const response = await api<{ data: Agent[] }>(`/api/v1/tenants/${tenantId}/agents`);
  return response.data;
}

export async function getAgent(tenantId: string, agentId: string) {
  const response = await api<{ data: Agent }>(`/api/v1/tenants/${tenantId}/agents/${agentId}`);
  return response.data;
}

// --- Agent Registration Tokens ---

export async function listAgentTokens(tenantId: string) {
  const response = await api<{ data: TokenRecord[] }>(`/api/v1/tenants/${tenantId}/agent-registration-tokens`);
  return response.data;
}

export async function createAgentToken(tenantId: string, name?: string) {
  return api<{ data: TokenRecord; token: string }>(`/api/v1/tenants/${tenantId}/agent-registration-tokens`, {
    method: 'POST',
    body: JSON.stringify({ name: name ?? 'Agent registration token' }),
  });
}

export async function revokeAgentToken(tenantId: string, tokenId: string) {
  return api<void>(`/api/v1/tenants/${tenantId}/agent-registration-tokens/${tokenId}/revoke`, {
    method: 'POST',
  });
}

