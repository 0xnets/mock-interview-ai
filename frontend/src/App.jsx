import { useEffect, useRef, useState } from 'react';
import { Routes, Route, Navigate, useNavigate, useLocation, useSearchParams } from 'react-router-dom';
import { AuthProvider, useAuth } from './providers/AuthProvider.jsx';
import { AppStateProvider, useAppState } from './providers/AppStateProvider.jsx';
import { ToastProvider, useToast } from './providers/ToastProvider.jsx';
import { AppShell } from './components/AppShell.jsx';
import { LoadingScreen } from './screens/LoadingScreen.jsx';
import { LoginScreen } from './screens/LoginScreen.jsx';
import { AcceptInviteScreen } from './screens/AcceptInviteScreen.jsx';
import { SetupScreen } from './screens/SetupScreen.jsx';
import { AdminInvitesScreen } from './screens/AdminInvitesScreen.jsx';
import { CandidateWelcomeScreen } from './screens/CandidateWelcomeScreen.jsx';
import { InterviewScreen } from './screens/InterviewScreen.jsx';
import { ResultsScreen } from './screens/ResultsScreen.jsx';
import { TranscriptScreen } from './screens/TranscriptScreen.jsx';
import { readCandidateSessionFromURL } from './candidate/session-codec.js';
import { ensureVoicesLoaded } from './voice/voice-selection.js';
import { refresh } from './api/client.js';
import { isAuthenticated } from './app/auth-store.js';

function RequireAuth({ children }) {
  const { isAuthenticated: authed } = useAuth();
  return authed ? children : <Navigate to="/login" replace />;
}

function RequireAdmin({ children }) {
  const { isAuthenticated: authed, hasRole } = useAuth();
  if (!authed) return <Navigate to="/login" replace />;
  return hasRole(['admin', 'super_admin']) ? children : <Navigate to="/setup" replace />;
}

function RequireCandidateLink({ children }) {
  const { isAuthenticated: authed } = useAuth();
  const [params] = useSearchParams();
  const sessionCode = params.get('session') || '';

  return authed || sessionCode ? children : <Navigate to="/login" replace />;
}

/// Boot dispatcher (replaces bootstrap.js). Reads the legacy query-param deep
/// links once, then routes to the matching screen.
function Boot() {
  const navigate = useNavigate();
  const location = useLocation();
  const [params] = useSearchParams();
  const { updateInterview } = useAppState();
  const toast = useToast();
  const ranRef = useRef(false);

  const inviteToken = params.get('invite') || params.get('accept_invite') || params.get('token') || '';
  const code = params.get('session') || '';
  const mode = inviteToken ? 'invite' : (code ? 'candidate' : 'hr');

  const [subtitle, setSubtitle] = useState(
    mode === 'candidate'
      ? 'Waiting for the backend to finish preparing personalized questions'
      : ''
  );

  useEffect(() => {
    if (ranRef.current) return;
    ranRef.current = true;
    (async () => {
      if (inviteToken) {
        navigate(`/accept-invite${location.search}`, { replace: true });
        return;
      }
      if (code) {
        const loaded = await readCandidateSessionFromURL({
          onWaiting: ({ state, attempt }) => setSubtitle(
            state === 'pending'
              ? `Preparing personalized questions (attempt ${attempt})…`
              : `Status: ${state}`
          ),
        });
        if (loaded) {
          updateInterview({
            sessionId: loaded.session.id || '',
            shortcode: loaded.shortcode || '',
            candidateName: loaded.session.candidate_name,
            role: loaded.session.role_title,
            linkExpired: loaded.session.state === 'expired',
            results: null,
            reportPdfUrl: '',
            reportPdfError: '',
          });
          navigate(`/welcome${location.search}`, { replace: true });
          ensureVoicesLoaded();
        } else {
          toast.error('This interview link could not be loaded. Please check the link or ask HR for a new one.');
          navigate('/setup', { replace: true });
        }
        return;
      }
      // HR flow: try a silent refresh first; on failure show login.
      let resumed = false;
      try { await refresh(); resumed = true; } catch { resumed = false; }
      navigate(resumed || isAuthenticated() ? '/setup' : '/login', { replace: true });
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  if (mode === 'candidate') {
    return <LoadingScreen title="Loading your interview..." subtitle={subtitle} />;
  }
  return <LoadingScreen title="Loading…" subtitle="" />;
}

export function App() {
  return (
    <AuthProvider>
      <AppStateProvider>
        <ToastProvider>
          <AppShell>
            <Routes>
              <Route path="/" element={<Boot />} />
              <Route path="/login" element={<LoginScreen />} />
              <Route path="/accept-invite" element={<AcceptInviteScreen />} />
              <Route path="/setup" element={<RequireAuth><SetupScreen /></RequireAuth>} />
              <Route path="/invites" element={<RequireAdmin><AdminInvitesScreen /></RequireAdmin>} />
              <Route path="/welcome" element={<RequireCandidateLink><CandidateWelcomeScreen /></RequireCandidateLink>} />
              <Route path="/interview" element={<RequireCandidateLink><InterviewScreen /></RequireCandidateLink>} />
              <Route path="/results" element={<RequireCandidateLink><ResultsScreen /></RequireCandidateLink>} />
              <Route path="/transcript" element={<RequireCandidateLink><TranscriptScreen /></RequireCandidateLink>} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </AppShell>
        </ToastProvider>
      </AppStateProvider>
    </AuthProvider>
  );
}
