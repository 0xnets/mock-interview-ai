import { createContext, useCallback, useContext, useMemo, useRef, useState } from 'react';

const ToastContext = createContext(null);
const DEFAULT_DURATION_MS = 4500;

function normalizeToast(input, fallbackTone) {
  if (typeof input === 'string') {
    return { message: input, tone: fallbackTone };
  }
  return {
    message: input?.message || '',
    tone: input?.tone || fallbackTone,
    duration: input?.duration,
  };
}

export function ToastProvider({ children }) {
  const [toasts, setToasts] = useState([]);
  const nextIdRef = useRef(1);

  const dismiss = useCallback((id) => {
    setToasts(prev => prev.filter(toast => toast.id !== id));
  }, []);

  const push = useCallback((input, fallbackTone = 'info') => {
    const toast = normalizeToast(input, fallbackTone);
    if (!toast.message) return null;

    const id = nextIdRef.current++;
    const duration = Number.isFinite(toast.duration) ? toast.duration : DEFAULT_DURATION_MS;
    setToasts(prev => [...prev, { ...toast, id }]);

    if (duration > 0) {
      window.setTimeout(() => dismiss(id), duration);
    }
    return id;
  }, [dismiss]);

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
      <div className="toast-viewport" role="status" aria-live="polite" aria-atomic="true">
        {toasts.map(toast => (
          <div key={toast.id} className={`toast toast-${toast.tone}`}>
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
