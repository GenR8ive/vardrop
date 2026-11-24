import React from "react";
import { Navigate, Route, Routes, NavLink } from "react-router-dom";
import { useAuth } from "./auth";
import Layout from "./components/Layout";
import Login from "./pages/Login";
import Concept from "./pages/Concept";
import ClusterDetail from "./pages/ClusterDetail";
import EntryEdit from "./pages/EntryEdit";
import Users from "./pages/Users";
import Tokens from "./pages/Tokens";
import Audit from "./pages/Audit";
import Access from "./pages/Access";

export default function App() {
  const { user, loading } = useAuth();

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center text-slate-500">
        yükleniyor…
      </div>
    );
  }

  if (!user) {
    return (
      <Routes>
        <Route path="/login" element={<Login />} />
        <Route path="*" element={<Navigate to="/login" replace />} />
      </Routes>
    );
  }

  return (
    <Layout>
      <Routes>
        <Route path="/" element={<Concept />} />
        <Route path="/clusters/:id" element={<ClusterDetail />} />
        <Route path="/entries/:id" element={<EntryEdit />} />
        <Route path="/users" element={<Users />} />
        <Route path="/tokens" element={<Tokens />} />
        <Route path="/audit" element={<Audit />} />
        <Route path="/access" element={<Access />} />
        <Route path="/login" element={<Navigate to="/" replace />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </Layout>
  );
}

export function NavItem({ to, children }: { to: string; children: React.ReactNode }) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        `px-3 py-2 rounded-md text-sm transition-colors ${
          isActive ? "bg-slate-700/60 text-white" : "text-slate-400 hover:text-white hover:bg-slate-800"
        }`
      }
    >
      {children}
    </NavLink>
  );
}
