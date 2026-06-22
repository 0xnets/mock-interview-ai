import { useNavigate, useLocation } from 'react-router-dom';
import { useAuth } from '../providers/AuthProvider.jsx';
import { logout } from '../api/client.js';

/// Page chrome: the title header + auth nav, wrapped around every screen.
/// When unauthenticated, no auth buttons render (matches `renderAuthChrome`).
export function AppShell({ children }) {
  const { isAuthenticated, principal, hasRole } = useAuth();
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const showInvitesButton = pathname === '/setup' && hasRole(['admin', 'super_admin']);

  async function handleLogout() {
    try { await logout(); } catch { /* logout() clears the session in its finally */ }
    navigate('/login');
  }

  return (
    <div className="max-w-5xl mx-auto py-8 px-4">
      <header className="mb-6 flex items-center justify-between">
        <div>
          <h1 className="text-3xl font-bold text-gray-900">AI Mock Interview Platform</h1>
          <p className="text-gray-600 mt-1">Verbal mock interviews for candidates — powered by AI</p>
        </div>
        <div className="flex items-center gap-2">
          {isAuthenticated && (
            <>
              <span className="text-xs text-gray-600">
                {principal && `${principal.display_name || principal.account_id} (${principal.role})`}
              </span>
              {showInvitesButton && (
                <button
                  className={`btn-secondary text-sm ${pathname === '/invites' ? 'is-active' : ''}`}
                  aria-pressed={pathname === '/invites'}
                  onClick={() => navigate('/invites')}
                >👥 Invites</button>
              )}
              <button
                className={`btn-secondary text-sm ${pathname === '/setup' ? 'is-active' : ''}`}
                aria-pressed={pathname === '/setup'}
                onClick={() => navigate('/setup')}
              >⚙️ HR Settings</button>
              <button className="btn-secondary text-sm" onClick={handleLogout}>🚪 Logout</button>
            </>
          )}
        </div>
      </header>
      {children}
    </div>
  );
}
