import { api } from './client';
import type { BrowserInstance, LoginResponse, TokenRecord } from './types';

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

export async function listCdpTokens(tenantId: string) {
  const response = await api<{ data: TokenRecord[] }>(`/api/v1/tenants/${tenantId}/tokens/cdp`);
  return response.data;
}

export async function createCdpToken(tenantId: string) {
  return api<{ data: TokenRecord; token: string }>(`/api/v1/tenants/${tenantId}/tokens/cdp`, {
    method: 'POST',
    body: JSON.stringify({ name: 'Browser Detail token' }),
  });
}

