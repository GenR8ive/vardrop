//! Security policy: roller, token kapsamları ve varsayılan yetki matrisi.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Editor,
    Viewer,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Editor => "editor",
            Role::Viewer => "viewer",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "admin" => Role::Admin,
            "editor" => Role::Editor,
            "viewer" => Role::Viewer,
            _ => return None,
        })
    }

    /// Rolün taşıyabileceği en geniş kapsam kümesi.
    pub fn allowed_scopes(&self) -> &'static [Scope] {
        match self {
            Role::Admin => &[Scope::Read, Scope::Write, Scope::Admin],
            Role::Editor => &[Scope::Read, Scope::Write],
            Role::Viewer => &[Scope::Read],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Read,
    Write,
    Admin,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Read => "read",
            Scope::Write => "write",
            Scope::Admin => "admin",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "read" => Scope::Read,
            "write" => Scope::Write,
            "admin" => Scope::Admin,
            _ => return None,
        })
    }
}

// ---------------- RBAC: yetki anahtarları ----------------

/// RBAC yetki kataloğu. Roller bunların bir alt kümesini taşır.
/// Yeni anahtar eklemek listeyi genişletmek kadar basit; eski anahtarlar
/// veritabanında kalmışsa geçersiz sayılıp sessizce yok sayılır.
pub const PERMISSIONS: &[&str] = &[
    "cluster.view",
    "cluster.manage",
    "entry.view",
    "entry.create",
    "entry.edit",
    "entry.delete",
    "secret.read",
    // yalnız global geçerli (cluster'a özgü atanmaları anlamsız):
    "users.manage",
    "roles.manage",
    "audit.read",
];

/// Yetki anahtarı geçerli mi?
pub fn valid_permission(s: &str) -> bool {
    PERMISSIONS.contains(&s)
}

/// listedeki geçersiz anahtarları ayıklar; boş/tekrar da temizlenir.
pub fn sanitize_permissions(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in list {
        let t = p.trim();
        if valid_permission(t) && !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix() {
        assert_eq!(Role::Viewer.allowed_scopes().len(), 1);
        assert_eq!(Role::Admin.allowed_scopes().len(), 3);
        assert_eq!(Role::parse("editor"), Some(Role::Editor));
        assert_eq!(Scope::parse("read"), Some(Scope::Read));
    }

    #[test]
    fn permissions() {
        assert!(valid_permission("cluster.view"));
        assert!(!valid_permission("cluster.delete"));
        assert_eq!(
            sanitize_permissions(&["entry.view".into(), "nope".into(), "entry.view".into()]),
            vec!["entry.view".to_string()]
        );
    }
}
