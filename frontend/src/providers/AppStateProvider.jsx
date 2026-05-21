import { createContext, useContext, useState, useCallback, useEffect } from 'react';
import { envNumber } from '../app/env.js';
import { setSavedVoiceName } from '../voice/voice-selection.js';
import { useAuth } from './AuthProvider.jsx';
import { getHrConfig, updateHrConfig } from '../api/client.js';

const AppStateContext = createContext(null);

/// Roles whose accounts carry a reusable HR configuration.
const HR_ROLES = ['hr', 'admin', 'super_admin'];

function defaultConfig() {
  return {
    hrEmail: '',
    techCount: envNumber('DEFAULT_TECH_QUESTION_COUNT'),
    nonTechCount: 0,
    nonTechBank: '',
    passThreshold: envNumber('DEFAULT_PASS_THRESHOLD'),
    voiceName: '',
  };
}

/// Interview data that must survive a screen change (loading → welcome →
/// interview → results → transcript). Per-question runtime state stays local
/// to InterviewScreen.
const emptyInterview = {
  sessionId: '',
  shortcode: '',
  candidateName: '',
  role: '',
  joinNonce: '',
  wsPath: '/v1/ws/interview',
  linkExpired: false,
  results: null,
  reportPdfUrl: '',
  reportPdfError: '',
};

export function AppStateProvider({ children }) {
  const { isAuthenticated, principal } = useAuth();

  // HR config is account-scoped and lives on the backend. It starts as
  // defaults and is replaced once an authenticated HR session loads it.
  const [config, setConfig] = useState(defaultConfig);
  // idle | loading | ready | saving | error
  const [configStatus, setConfigStatus] = useState('idle');
  const [hasSavedConfig, setHasSavedConfig] = useState(false);

  const [interview, setInterview] = useState(emptyInterview);

  // Keep the framework-agnostic speak() voice resolution in sync with config.
  useEffect(() => { setSavedVoiceName(config.voiceName); }, [config.voiceName]);

  const loadConfig = useCallback(async () => {
    setConfigStatus('loading');
    try {
      const { config: loaded, saved } = await getHrConfig();
      setConfig(loaded);
      setHasSavedConfig(saved);
      setConfigStatus('ready');
      return loaded;
    } catch (e) {
      setConfigStatus('error');
      throw e;
    }
  }, []);

  const saveConfig = useCallback(async (next) => {
    setConfigStatus('saving');
    try {
      const { config: saved } = await updateHrConfig(next);
      setConfig(saved);
      setHasSavedConfig(true);
      setConfigStatus('ready');
      return saved;
    } catch (e) {
      // A failed save does not invalidate the already-loaded config.
      setConfigStatus('ready');
      throw e;
    }
  }, []);

  // Load the account's HR config once an authenticated HR/admin/super_admin
  // session is available; reset to defaults when the session ends.
  const role = principal?.role;
  useEffect(() => {
    if (isAuthenticated && HR_ROLES.includes(role)) {
      loadConfig().catch(() => { /* surfaced via configStatus */ });
    } else {
      setConfig(defaultConfig());
      setHasSavedConfig(false);
      setConfigStatus('idle');
    }
  }, [isAuthenticated, role, loadConfig]);

  const updateInterview = useCallback((patch) => {
    setInterview(prev => ({ ...prev, ...patch }));
  }, []);

  const resetInterview = useCallback(() => setInterview(emptyInterview), []);

  const value = {
    config,
    configStatus,
    hasSavedConfig,
    loadConfig,
    saveConfig,
    interview,
    updateInterview,
    resetInterview,
  };
  return <AppStateContext.Provider value={value}>{children}</AppStateContext.Provider>;
}

export function useAppState() {
  return useContext(AppStateContext);
}
