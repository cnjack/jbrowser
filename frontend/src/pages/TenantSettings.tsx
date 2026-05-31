import React, { FormEvent, useState, useEffect, useRef } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { Copy, Trash2, UserMinus } from 'lucide-react';
import {
  listMembers,
  updateMemberRole,
  removeMember,
  createInvitation,
  listInvitations,
  revokeInvitation,
  updateTenant,
  deleteTenant,
} from '../api/client';
import type { Member, Invitation } from '../api/client';
import { Sidebar } from '../components/Sidebar';
import { useAuthStore } from '../stores/auth';

interface Props {
  tenantId: string;
}

export function TenantSettings({ tenantId }: Props) {
  const queryClient = useQueryClient();
  const currentTenant = useAuthStore((s) => s.getCurrentTenant());
  const userEmail = useAuthStore((s) => s.userEmail);
  const isAdmin = useAuthStore((s) => s.isAdmin());

  // Scroll to members section when #members hash is present (on mount or same-page hash change)
  const membersRef = useRef<HTMLElement>(null);
  useEffect(() => {
    function scrollToMembers() {
      membersRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
    if (window.location.hash === '#members') {
      setTimeout(scrollToMembers, 100);
    }
    function handleHashChange() {
      if (window.location.hash === '#members') {
        scrollToMembers();
      }
    }
    window.addEventListener('hashchange', handleHashChange);
    return () => window.removeEventListener('hashchange', handleHashChange);
  }, []);

  return (
    <div className="app-layout">
      <Sidebar activePage="settings" tenantId={tenantId} />
      <main className="main-area">
        <div className="page-shell">
          <div className="page-header">
            <div>
              <h1 className="page-title">Settings</h1>
              <p className="page-subtitle">
                Manage your tenant settings, members, and invitations.
              </p>
            </div>
          </div>

          <div className="settings-sections">
            <GeneralSection
              tenantId={tenantId}
              tenantName={currentTenant?.name ?? ''}
              isAdmin={isAdmin}
              queryClient={queryClient}
            />
            <MembersSection
              tenantId={tenantId}
              isAdmin={isAdmin}
              currentUserEmail={userEmail}
              queryClient={queryClient}
              sectionRef={membersRef}
            />
            {isAdmin && (
              <DangerSection tenantId={tenantId} tenantName={currentTenant?.name ?? ''} />
            )}
          </div>
        </div>
      </main>
    </div>
  );
}

// ── General Section ──────────────────────────────────────────────────────────

function GeneralSection({
  tenantId,
  tenantName,
  isAdmin,
  queryClient,
}: {
  tenantId: string;
  tenantName: string;
  isAdmin: boolean;
  queryClient: ReturnType<typeof useQueryClient>;
}) {
  const [name, setName] = useState(tenantName);
  const [saved, setSaved] = useState(false);

  const mutation = useMutation({
    mutationFn: () => updateTenant(tenantId, name),
    onSuccess: () => {
      setSaved(true);
      queryClient.invalidateQueries({ queryKey: ['tenant', tenantId] });
      setTimeout(() => setSaved(false), 2000);
    },
  });

  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    mutation.mutate();
  }

  return (
    <section className="settings-section">
      <h2 className="settings-section-title">General</h2>
      <div className="settings-card">
        <form onSubmit={handleSubmit}>
          <div className="field">
            <label htmlFor="tenantName">Tenant Name</label>
            <input
              id="tenantName"
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={!isAdmin}
              required
            />
          </div>
          {isAdmin && (
            <button type="submit" className="btn-primary" disabled={mutation.isPending}>
              {mutation.isPending ? 'Saving…' : saved ? 'Saved!' : 'Save changes'}
            </button>
          )}
          {mutation.isError && (
            <div className="error-msg" style={{ marginTop: 12 }}>
              {mutation.error instanceof Error ? mutation.error.message : 'Failed to update'}
            </div>
          )}
        </form>
      </div>
    </section>
  );
}

// ── Members Section ──────────────────────────────────────────────────────────

function MembersSection({
  tenantId,
  isAdmin,
  currentUserEmail,
  queryClient,
  sectionRef,
}: {
  tenantId: string;
  isAdmin: boolean;
  currentUserEmail: string | null;
  queryClient: ReturnType<typeof useQueryClient>;
  sectionRef?: React.Ref<HTMLElement>;
}) {
  const { data: membersData, isLoading: membersLoading } = useQuery({
    queryKey: ['members', tenantId],
    queryFn: () => listMembers(tenantId),
  });

  const { data: invitationsData } = useQuery({
    queryKey: ['invitations', tenantId],
    queryFn: () => listInvitations(tenantId),
    enabled: isAdmin,
  });

  const members: Member[] = membersData?.data ?? [];
  const invitations: Invitation[] = invitationsData?.data ?? [];

  const roleMutation = useMutation({
    mutationFn: ({ userId, role }: { userId: string; role: string }) =>
      updateMemberRole(tenantId, userId, role),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['members', tenantId] }),
  });

  const removeMutation = useMutation({
    mutationFn: (userId: string) => removeMember(tenantId, userId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['members', tenantId] }),
  });

  return (
    <section className="settings-section" id="members" ref={sectionRef}>
      <h2 className="settings-section-title">Members</h2>
      <div className="settings-card">
        {membersLoading ? (
          <div className="loading-state">Loading members…</div>
        ) : (
          <div className="table-wrap" style={{ marginTop: 0 }}>
            <table className="data-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Email</th>
                  <th>Role</th>
                  {isAdmin && <th style={{ width: 80 }}>Actions</th>}
                </tr>
              </thead>
              <tbody>
                {members.map((m) => (
                  <tr key={m.user_id}>
                    <td>{m.display_name}</td>
                    <td>{m.email}</td>
                    <td>
                      {isAdmin && m.email !== currentUserEmail ? (
                        <select
                          className="role-select"
                          value={m.role}
                          onChange={(e) =>
                            roleMutation.mutate({ userId: m.user_id, role: e.target.value })
                          }
                        >
                          <option value="admin">Admin</option>
                          <option value="member">Member</option>
                        </select>
                      ) : (
                        <span className={`role-badge role-${m.role}`}>{m.role}</span>
                      )}
                    </td>
                    {isAdmin && (
                      <td>
                        {m.email !== currentUserEmail && (
                          <button
                            type="button"
                            className="icon-btn-sm danger"
                            title="Remove member"
                            onClick={() => {
                              if (confirm(`Remove ${m.display_name} from this tenant?`)) {
                                removeMutation.mutate(m.user_id);
                              }
                            }}
                          >
                            <UserMinus size={14} />
                          </button>
                        )}
                      </td>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {isAdmin && (
        <>
          <h3 className="settings-subsection-title">Invitations</h3>
          <div className="settings-card">
            <InviteCreator tenantId={tenantId} queryClient={queryClient} />
            {invitations.length > 0 && (
              <div className="table-wrap">
                <table className="data-table">
                  <thead>
                    <tr>
                      <th>Role</th>
                      <th>Link</th>
                      <th>Created</th>
                      <th style={{ width: 80 }}>Actions</th>
                    </tr>
                  </thead>
                  <tbody>
                    {invitations.map((inv) => (
                      <InviteRow
                        key={inv.id}
                        invite={inv}
                        tenantId={tenantId}
                        queryClient={queryClient}
                      />
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </>
      )}
    </section>
  );
}

function InviteCreator({
  tenantId,
  queryClient,
}: {
  tenantId: string;
  queryClient: ReturnType<typeof useQueryClient>;
}) {
  const [role, setRole] = useState('member');
  const [generatedLink, setGeneratedLink] = useState<string | null>(null);

  const mutation = useMutation({
    mutationFn: () => createInvitation(tenantId, role),
    onSuccess: (res) => {
      const link = `${window.location.origin}/invite/${res.data.token}`;
      setGeneratedLink(link);
      queryClient.invalidateQueries({ queryKey: ['invitations', tenantId] });
    },
  });

  return (
    <div className="invite-creator">
      <div className="invite-creator-row">
        <select className="role-select" value={role} onChange={(e) => setRole(e.target.value)}>
          <option value="member">Member</option>
          <option value="admin">Admin</option>
        </select>
        <button
          type="button"
          className="btn-primary"
          onClick={() => mutation.mutate()}
          disabled={mutation.isPending}
        >
          {mutation.isPending ? 'Creating…' : 'Create Invite Link'}
        </button>
      </div>
      {generatedLink && (
        <div className="invite-link-box">
          <code className="invite-link-text">{generatedLink}</code>
          <button
            type="button"
            className="agent-copy-btn"
            onClick={() => navigator.clipboard.writeText(generatedLink)}
          >
            <Copy size={13} /> Copy
          </button>
        </div>
      )}
      {mutation.isError && (
        <div className="error-msg" style={{ marginTop: 8 }}>
          {mutation.error instanceof Error ? mutation.error.message : 'Failed to create invitation'}
        </div>
      )}
    </div>
  );
}

function InviteRow({
  invite,
  tenantId,
  queryClient,
}: {
  invite: Invitation;
  tenantId: string;
  queryClient: ReturnType<typeof useQueryClient>;
}) {
  const revokeMutation = useMutation({
    mutationFn: () => revokeInvitation(tenantId, invite.id),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['invitations', tenantId] }),
  });

  const link = `${window.location.origin}/invite/${invite.token}`;

  return (
    <tr>
      <td><span className={`role-badge role-${invite.role}`}>{invite.role}</span></td>
      <td>
        <div className="invite-cell-link">
          <code className="invite-token-inline">{link.slice(0, 50)}…</code>
          <button
            type="button"
            className="icon-btn-sm"
            title="Copy link"
            onClick={() => navigator.clipboard.writeText(link)}
          >
            <Copy size={12} />
          </button>
        </div>
      </td>
      <td>{new Date(invite.created_at).toLocaleDateString()}</td>
      <td>
        <button
          type="button"
          className="icon-btn-sm danger"
          title="Revoke"
          onClick={() => revokeMutation.mutate()}
          disabled={revokeMutation.isPending}
        >
          <Trash2 size={14} />
        </button>
      </td>
    </tr>
  );
}

// ── Danger Section ───────────────────────────────────────────────────────────

function DangerSection({ tenantId, tenantName }: { tenantId: string; tenantName: string }) {
  const [confirmName, setConfirmName] = useState('');
  const logout = useAuthStore((s) => s.logout);

  const mutation = useMutation({
    mutationFn: () => deleteTenant(tenantId),
    onSuccess: () => {
      logout();
      window.location.href = '/login';
    },
  });

  return (
    <section className="settings-section">
      <h2 className="settings-section-title">Danger Zone</h2>
      <div className="settings-card danger-card">
        <h3>Delete this tenant</h3>
        <p>
          Once you delete a tenant, there is no going back. All browsers, agents, and data will be
          permanently removed.
        </p>
        <div className="field">
          <label htmlFor="confirmDelete">
            Type <strong>{tenantName}</strong> to confirm
          </label>
          <input
            id="confirmDelete"
            type="text"
            value={confirmName}
            onChange={(e) => setConfirmName(e.target.value)}
            placeholder={tenantName}
          />
        </div>
        <button
          type="button"
          className="btn-danger"
          disabled={confirmName !== tenantName || mutation.isPending}
          onClick={() => mutation.mutate()}
        >
          {mutation.isPending ? 'Deleting…' : 'Delete Tenant'}
        </button>
        {mutation.isError && (
          <div className="error-msg" style={{ marginTop: 12 }}>
            {mutation.error instanceof Error ? mutation.error.message : 'Failed to delete tenant'}
          </div>
        )}
      </div>
    </section>
  );
}
