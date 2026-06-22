import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from 'react';

const ToastContext = createContext(null);
const DEFAULT_DURATION_MS = 4500;
const MAX_VISIBLE_TOASTS = 4;
const TONES = new Set(['info', 'success', 'warning', 'error']);

function normalizeToast(input, fallbackTone) {
  if (typeof input === 'string') {
    return {
      message: input,
      tone: normalizeTone(fallbackTone),
      duration: DEFAULT_DURATION_MS,
    };
  }
  return {
    message: String(input?.message || ''),
    tone: normalizeTone(input?.tone || fallbackTone),
    duration: normalizeDuration(input?.duration),
  };
}

function normalizeTone(tone) {
  return TONES.has(tone) ? tone : 'info';
}

function normalizeDuration(duration) {
  if (duration == null) return DEFAULT_DURATION_MS;
  const numeric = Number(duration);
  return Number.isFinite(numeric) ? Math.max(0, numeric) : DEFAULT_DURATION_MS;
}

export function ToastProvider({ children }) {
  const [toasts, setToasts] = useState([]);
  const nextIdRef = useRef(1);
  const timersRef = useRef(new Map());

  const dismiss = useCallback((id) => {
    const timer = timersRef.current.get(id);
    if (timer) window.clearTimeout(timer);
    timersRef.current.delete(id);
    setToasts(prev => prev.filter(toast => toast.id !== id));
  }, []);

  const push = useCallback((input, fallbackTone = 'info') => {
    const toast = normalizeToast(input, fallbackTone);
    if (!toast.message) return null;

    const id = nextIdRef.current++;
    const duration = toast.duration;
    setToasts(prev => {
      const next = [...prev, { ...toast, id }];
      const overflow = Math.max(0, next.length - MAX_VISIBLE_TOASTS);
      if (overflow === 0) return next;
      for (const dropped of next.slice(0, overflow)) {
        const timer = timersRef.current.get(dropped.id);
        if (timer) window.clearTimeout(timer);
        timersRef.current.delete(dropped.id);
      }
      return next.slice(overflow);
    });

    if (duration > 0) {
      const timer = window.setTimeout(() => dismiss(id), duration);
      timersRef.current.set(id, timer);
    }
    return id;
  }, [dismiss]);

  useEffect(() => () => {
    for (const timer of timersRef.current.values()) {
      window.clearTimeout(timer);
    }
    timersRef.current.clear();
  }, []);

  const value = useMemo(() => ({
    show: push,
    dismiss,
    info: input => push(input, 'info'),
    success: input => push(input, 'success'),
    warning: input => push(input, 'warning'),
    error: input => push(input, 'error'),
  }), [dismiss, push]);

  return (
    <ToastContext.Provider value={value}>
      {children}
      <div className="toast-viewport" aria-live="polite" aria-atomic="false">
        {toasts.map(toast => (
          <div
            key={toast.id}
            className={`toast toast-${toast.tone}`}
            role={toast.tone === 'error' ? 'alert' : 'status'}
          >
            <div className="toast-content">
              <div className="toast-title">{toast.tone}</div>
              <div className="toast-message">{toast.message}</div>
            </div>
            <button
              className="toast-close"
              type="button"
              aria-label="Dismiss notification"
              onClick={() => dismiss(toast.id)}
            >
              x
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  );
}

export function useToast() {
  const context = useContext(ToastContext);
  if (!context) {
    throw new Error('useToast must be used inside ToastProvider');
  }
  return context;
}
