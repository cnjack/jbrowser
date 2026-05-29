export interface Tenant {
  id: string;
  name: string;
  slug: string;
}

export interface BrowserTab {
  id: string;
  title: string;
  url: string;
  active: boolean;
  favicon_url?: string;
}

export interface BrowserInstance {
  id: string;
  tenant_id: string;
  agent_id: string;
  name: string;
  status: 'online' | 'offline' | 'unhealthy' | 'restarting';
  browser_type: string;
  browser_version: string;
  active_tab_id?: string | null;
  tabs: BrowserTab[];
  proxy_enabled: boolean;
  viewport_width: number;
  viewport_height: number;
  viewer_count: number;
  agent_name: string;
  agent_status: 'online' | 'offline' | 'unhealthy';
  last_heartbeat_at?: string | null;
}

export interface TokenRecord {
  id: string;
  tenant_id: string;
  token_type: 'agent_registration' | 'agent_runtime' | 'tenant_cdp_access';
  name?: string | null;
  token_prefix: string;
  revoked_at?: string | null;
  created_at: string;
}

export interface LoginResponse {
  access_token: string;
  token_type: 'Bearer';
  user: {
    id: string;
    email: string;
    display_name: string;
  };
  tenants: Tenant[];
}

