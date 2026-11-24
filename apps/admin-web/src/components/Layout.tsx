import React from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { useAuth } from "../auth";
import { FeedbackProvider } from "./Feedback";
import InstallButton from "./InstallButton";
import { usePerms } from "../perms";

const navItems = [
  { to: "/", label: "Concept", end: true, perm: null },
  { to: "/tokens", label: "Tokenlar", perm: null },
  { to: "/users", label: "Users", perm: "users.manage" },
  { to: "/access", label: "Erişim", perm: "roles.manage" },
  { to: "/audit", label: "Audit", perm: "audit.read" },
];

export default function Layout({ children }: { children: React.ReactNode }) {
  const { user, logout } = useAuth();
  const navigate = useNavigate();
  const { can } = usePerms();

  return (
    <div className="app-shell flex">
      <aside className="app-sidebar w-56 shrink-0 bg-white border-r border-slate-800 flex flex-col">
        <div className="px-4 py-4 border-b border-slate-800 flex items-center gap-2.5">
          <img src="/logo.png" alt="Vardrop" className="h-7 w-7 rounded-md" />
          <div>
            <div className="text-base font-semibold tracking-tight text-white leading-tight">Vardrop</div>
            <div className="text-xs text-slate-500">Registry</div>
          </div>
        </div>
        <nav className="flex-1 p-2 space-y-1">
          {navItems
            .filter((n) => !n.perm || can(n.perm))
            .map((n) => (
            <NavLink
              key={n.to}
              to={n.to}
              end={n.end}
              className={({ isActive }) =>
                `block px-3 py-2 rounded-md text-sm ${
                  isActive
                    ? "bg-slate-800 text-white font-medium"
                    : "text-slate-400 hover:text-white hover:bg-slate-800/60"
                }`
              }
            >
              {n.label}
            </NavLink>
          ))}
        </nav>
        <InstallButton />
        <div className="p-3 border-t border-slate-800">
          <div className="text-sm text-slate-300">{user?.display_name || user?.username}</div>
          <div className="text-xs text-slate-500 mb-2">{user?.role}</div>
          <button
            onClick={() => {
              logout();
              navigate("/login");
            }}
            className="w-full text-left text-xs text-slate-400 hover:text-rose-600"
          >
            Çıkış
          </button>
        </div>
      </aside>
      <FeedbackProvider>
        <main className="app-main flex-1 min-h-screen overflow-auto bg-slate-950">{children}</main>
      </FeedbackProvider>
    </div>
  );
}
