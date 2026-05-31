import { useAuthStore } from '../stores/auth';

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? '';

export class ApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: unknown,
  ) {
    super(extractMessage(body) ?? `API error ${status}`);
  }
}

export async function api<T>(path: string, options: RequestInit = {}): Promise<T> {
  const token = useAuthStore.getState().token;
  const response = await fetch(`${API_BASE_URL}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
      ...options.headers,
    },
  });

  if (!response.ok) {
    const body = await response.json().catch(() => null);
    if (response.status === 401) {
      useAuthStore.getState().logout();
      window.location.href = '/login';
    }
    throw new ApiError(response.status, body);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

function extractMessage(body: unknown): string | undefined {
  if (body && typeof body === 'object' && 'detail' in body && typeof body.detail === 'string') {
    return body.detail;
  }
  return undefined;
}

// ── Auth ──────────────────────────────────────────────────────────────────────

export function signup(email: string, password: string, displayName: string, inviteToken?: string) {
  const params = inviteToken ? `?invite_token=${encodeURIComponent(inviteToken)}` : '';
  return api<import('./types').LoginResponse>(`/api/v1/auth/signup${params}`, {
    method: 'POST',
    body: JSON.stringify({ email, password, display_name: displayName }),
  });
}

export function changePassword(currentPassword: string, newPassword: string) {
  return api<void>('/api/v1/auth/change-password', {
    method: 'POST',
    body: JSON.stringify({ current_password: currentPassword, new_password: newPassword }),
  });
}

// ── Members ───────────────────────────────────────────────────────────────────

export function listMembers(tenantId: string) {
  return api<{ data: Member[] }>(`/api/v1/tenants/${tenantId}/members`);
}

export function updateMemberRole(tenantId: string, userId: string, role: string) {
  return api<{ data: Member }>(`/api/v1/tenants/${tenantId}/members/${userId}`, {
    method: 'PATCH',
    body: JSON.stringify({ role }),
  });
}

export function removeMember(tenantId: string, userId: string) {
  return api<void>(`/api/v1/tenants/${tenantId}/members/${userId}`, {
    method: 'DELETE',
  });
}

export function leaveTenant(tenantId: string) {
  return api<void>(`/api/v1/tenants/${tenantId}/members/leave`, {
    method: 'POST',
  });
}

// ── Invitations ───────────────────────────────────────────────────────────────

export function createInvitation(tenantId: string, role: string) {
  return api<{ data: Invitation }>(`/api/v1/tenants/${tenantId}/invitations`, {
    method: 'POST',
    body: JSON.stringify({ role }),
  });
}

export function listInvitations(tenantId: string) {
  return api<{ data: Invitation[] }>(`/api/v1/tenants/${tenantId}/invitations`);
}

export function revokeInvitation(tenantId: string, invitationId: string) {
  return api<void>(`/api/v1/tenants/${tenantId}/invitations/${invitationId}`, {
    method: 'DELETE',
  });
}

// ── Invitation acceptance (public) ────────────────────────────────────────────

export function validateInvite(token: string) {
  return api<InviteValidation>(`/api/v1/invitations/${encodeURIComponent(token)}/validate`);
}

export function acceptInvite(token: string) {
  return api<import('./types').LoginResponse>(`/api/v1/invitations/${encodeURIComponent(token)}/accept`, {
    method: 'POST',
  });
}

// ── Tenant management ─────────────────────────────────────────────────────────

export function updateTenant(tenantId: string, name: string) {
  return api<{ data: import('./types').Tenant }>(`/api/v1/tenants/${tenantId}`, {
    method: 'PATCH',
    body: JSON.stringify({ name }),
  });
}

export function createTenant(name: string) {
  return api<{
    access_token: string;
    token_type: string;
    tenant: import('./types').Tenant;
    tenants: import('./types').Tenant[];
  }>('/api/v1/tenants', {
    method: 'POST',
    body: JSON.stringify({ name }),
  });
}

export function deleteTenant(tenantId: string) {
  return api<void>(`/api/v1/tenants/${tenantId}`, {
    method: 'DELETE',
  });
}

// ── Types for new API endpoints ───────────────────────────────────────────────

export interface Member {
  user_id: string;
  email: string;
  display_name: string;
  role: string;
  joined_at: string;
}

export interface Invitation {
  id: string;
  tenant_id: string;
  role: string;
  token: string;
  created_at: string;
  expires_at: string;
}

export interface InviteValidation {
  tenant_name: string;
  role: string;
  inviter_email?: string;
}

