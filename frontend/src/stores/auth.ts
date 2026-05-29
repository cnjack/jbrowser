import { create } from 'zustand';
import type { Tenant } from '../api/types';

interface AuthState {
  token: string | null;
  tenantId: string | null;
  userEmail: string | null;
  tenants: Tenant[];
  setSession: (token: string, userEmail: string, tenants: Tenant[]) => void;
  logout: () => void;
}

const storedToken = localStorage.getItem('jbrowser.token');
const storedTenantId = localStorage.getItem('jbrowser.tenantId');
const storedEmail = localStorage.getItem('jbrowser.userEmail');

export const useAuthStore = create<AuthState>((set) => ({
  token: storedToken,
  tenantId: storedTenantId,
  userEmail: storedEmail,
  tenants: [],
  setSession: (token, userEmail, tenants) => {
    const tenantId = tenants[0]?.id ?? null;
    localStorage.setItem('jbrowser.token', token);
    localStorage.setItem('jbrowser.userEmail', userEmail);
    if (tenantId) {
      localStorage.setItem('jbrowser.tenantId', tenantId);
    }
    set({ token, userEmail, tenants, tenantId });
  },
  logout: () => {
    localStorage.removeItem('jbrowser.token');
    localStorage.removeItem('jbrowser.tenantId');
    localStorage.removeItem('jbrowser.userEmail');
    set({ token: null, tenantId: null, userEmail: null, tenants: [] });
  },
}));

