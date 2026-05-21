import { createContext, useContext, useState, useCallback, useEffect } from 'react';
import { ENV, envNumber } from '../app/env.js';
import { setSavedVoiceName } from '../voice/voice-selection.js';

const AppStateContext = createContext(null);

function defaultConfig() {
  return {
    hrEmail: '',
    techCount: envNumber('DEFAULT_TECH_QUESTION_COUNT'),
    nonTechCount: envNumber('DEFAULT_NON_TECH_QUESTION_COUNT'),
    nonTechBank: '',
    passThreshold: envNumber('DEFAULT_PASS_THRESHOLD'),
    voiceName: '',
  };
}

function loadStoredConfig() {
  try {
    const saved = localStorage.getItem(ENV.LOCAL_STORAGE_CONFIG_KEY);
    if (saved) return { config: { ...defaultConfig(), ...JSON.parse(saved) }, hasSaved: true };
  } catch { /* ignore malformed storage */ }
  return { config: defaultConfig(), hasSaved: false };
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
  results: null,
  reportPdfUrl: '',
  reportPdfError: '',
};

export function AppStateProvider({ children }) {
  const [{ config, hasSaved }, setConfigState] = useState(() => {
    const loaded = loadStoredConfig();
    return { config: loaded.config, hasSaved: loaded.hasSaved };
  });
  const [interview, setInterview] = useState(emptyInterview);

  // Keep the framework-agnostic speak() voice resolution in sync with config.
  useEffect(() => { setSavedVoiceName(config.voiceName); }, [config.voiceName]);

  const saveConfig = useCallback((next) => {
    localStorage.setItem(ENV.LOCAL_STORAGE_CONFIG_KEY, JSON.stringify(next));
    setConfigState({ config: next, hasSaved: true });
  }, []);

  const updateInterview = useCallback((patch) => {
    setInterview(prev => ({ ...prev, ...patch }));
  }, []);

  const resetInterview = useCallback(() => setInterview(emptyInterview), []);

  const value = {
    config,
    hasSavedConfig: hasSaved,
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
