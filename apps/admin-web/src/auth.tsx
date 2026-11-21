import React, { createContext, useContext, useEffect, useState } from "react";
import { Api, getUser, setSession, clearSession } from "./api";
import type { User } from "./types";

interface AuthCtx {
  user: User | null;
  loading: boolean;
  login: (username: string, password: string) => Promise<void>;
  logout: () => void;
}

const Ctx = createContext<AuthCtx>({
  user: null,
  loading: true,
  login: async () => {},
  logout: () => {},
});

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<User | null>(getUser());
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const stored = getUser();
    if (!stored) {
      setLoading(false);
      return;
    }
    Api.me()
      .then(() => {
        setUser(stored);
        setLoading(false);
      })
      .catch(() => {
        clearSession();
        setUser(null);
        setLoading(false);
      });
  }, []);

  async function login(username: string, password: string) {
    const res = await Api.login(username, password);
    setSession(res.token, res.user);
    setUser(res.user);
  }

  function logout() {
    Api.logout();
    setUser(null);
  }

  return <Ctx.Provider value={{ user, loading, login, logout }}>{children}</Ctx.Provider>;
}

export function useAuth() {
  return useContext(Ctx);
}
