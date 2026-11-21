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
}
