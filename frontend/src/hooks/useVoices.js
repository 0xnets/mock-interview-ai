import { useEffect, useState } from 'react';
import { listEnglishVoices, ensureVoicesLoaded } from '../voice/voice-selection.js';

/// Grouped English voices for the config <select>, refreshed when the browser
/// finishes loading its voice list (`voiceschanged`).
export function useVoices() {
  const [groups, setGroups] = useState(() => listEnglishVoices());

  useEffect(() => {
    let active = true;
    const refresh = () => { if (active) setGroups(listEnglishVoices()); };
    refresh();
    const ss = window.speechSynthesis;
    if (ss) ss.addEventListener('voiceschanged', refresh);
    ensureVoicesLoaded().then(refresh);
    return () => {
      active = false;
      if (ss) ss.removeEventListener('voiceschanged', refresh);
    };
  }, []);

  return groups;
}
