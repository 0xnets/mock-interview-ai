import { createContext, useContext, useEffect, useState } from 'react';
import { subscribe, isAuthenticated, getPrincipal, hasRole } from '../app/auth-store.js';

const AuthContext = createContext(null);

/// Bridges the framework-agnostic in-memory auth store into React. The store
/// holds the access token in memory, schedules the silent refresh, and clears
/// the session on refresh failure — none of that logic changes here.
export function AuthProvider({ children }) {
  const [snapshot, setSnapshot] = useState(() => ({
    isAuthenticated: isAuthenticated(),
    principal: getPrincipal(),
  }));

  useEffect(() => {
    const update = () => setSnapshot({
      isAuthenticated: isAuthenticated(),
      principal: getPrincipal(),
    });
    update();
    return subscribe(update);
  }, []);

  const value = {
    isAuthenticated: snapshot.isAuthenticated,
    principal: snapshot.principal,
    hasRole,
  };
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  return useContext(AuthContext);
}
