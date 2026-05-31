import { create } from 'zustand';
import type { Tenant } from '../api/types';

interface User {
  id: string;
  email: string;
  display_name: string;
}

interface AuthState {
  token: string | null;
  tenantId: string | null;
  userEmail: string | null;
  user: User | null;
  tenants: Tenant[];
  setSession: (token: string, userEmail: string, tenants: Tenant[]) => void;
  setAuth: (token: string, user: User, tenants: Tenant[]) => void;
  setTenants: (token: string, tenants: Tenant[]) => void;
  switchTenant: (tenantId: string) => void;
  getCurrentTenant: () => (Tenant & { role?: string }) | null;
  isAdmin: () => boolean;
  logout: () => void;
}

const storedToken = localStorage.getItem('jbrowser.token');
const storedTenantId = localStorage.getItem('jbrowser.tenantId');
const storedEmail = localStorage.getItem('jbrowser.userEmail');
const storedTenants: Tenant[] = (() => {
  try {
    return JSON.parse(localStorage.getItem('jbrowser.tenants') ?? '[]');
  } catch {
    return [];
  }
})();

export const useAuthStore = create<AuthState>((set, get) => ({
  token: storedToken,
  tenantId: storedTenantId,
  userEmail: storedEmail,
  user: null,
  tenants: storedTenants,
  setSession: (token, userEmail, tenants) => {
    const stored = localStorage.getItem('jbrowser.tenantId');
    const tenantId = (stored && tenants.find((t) => t.id === stored))
      ? stored
      : tenants[0]?.id ?? null;
    localStorage.setItem('jbrowser.token', token);
    localStorage.setItem('jbrowser.userEmail', userEmail);
    localStorage.setItem('jbrowser.tenants', JSON.stringify(tenants));
    if (tenantId) {
      localStorage.setItem('jbrowser.tenantId', tenantId);
    }
    set({ token, userEmail, tenants, tenantId });
  },
  setAuth: (token, user, tenants) => {
    const stored = localStorage.getItem('jbrowser.tenantId');
    const tenantId = (stored && tenants.find((t) => t.id === stored))
      ? stored
      : tenants[0]?.id ?? null;
    localStorage.setItem('jbrowser.token', token);
    localStorage.setItem('jbrowser.userEmail', user.email);
    localStorage.setItem('jbrowser.tenants', JSON.stringify(tenants));
    if (tenantId) {
      localStorage.setItem('jbrowser.tenantId', tenantId);
    }
    set({ token, user, userEmail: user.email, tenants, tenantId });
  },
  switchTenant: (tenantId) => {
    localStorage.setItem('jbrowser.tenantId', tenantId);
    set({ tenantId });
  },
  setTenants: (token, tenants) => {
    localStorage.setItem('jbrowser.token', token);
    localStorage.setItem('jbrowser.tenants', JSON.stringify(tenants));
    set({ token, tenants });
  },
  getCurrentTenant: () => {
    const { tenants, tenantId } = get();
    if (!tenantId) return null;
    return tenants.find((t) => t.id === tenantId) ?? null;
  },
  isAdmin: () => {
    const tenant = get().getCurrentTenant();
    return (tenant as Tenant & { role?: string })?.role === 'admin';
  },
  logout: () => {
    localStorage.removeItem('jbrowser.token');
    localStorage.removeItem('jbrowser.tenantId');
    localStorage.removeItem('jbrowser.userEmail');
    localStorage.removeItem('jbrowser.tenants');
    set({ token: null, tenantId: null, userEmail: null, user: null, tenants: [] });
  },
}));

