//! GET /api/nav — popup istemcisinin boş-durumu (grup ağacıyla gezinme).
//! Tek çağrı: klusterler + tam grup hiyerarşisi + alt ağaç değişken sayıları.

use crate::auth::middleware::AuthUser;
use crate::error::ApiResult;
use crate::state::SharedState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use vardrop_core::{GroupNode, NavCluster};
use sqlx::Row;

pub fn router() -> Router<SharedState> {
    Router::new().route("/api/nav", get(nav))
}

#[derive(sqlx::FromRow)]
struct GRow {
    id: String,
    cluster_id: String,
    parent_id: Option<String>,
    name: String,
    cnt: i64,
}

pub async fn nav(user: AuthUser, State(state): State<SharedState>) -> ApiResult<Json<Vec<NavCluster>>> {
    let c_rows = sqlx::query(
        "SELECT id, name, slug, color FROM clusters ORDER BY name COLLATE NOCASE",
    )
    .fetch_all(&state.db)
    .await?;

    // RBAC: istemci yalnız entry.view yetkisi olan cluster'ları görür.
    let c_rows: Vec<_> = c_rows
        .into_iter()
        .filter(|c| {
            let cid: String = c.get("id");
            user.perms.can_on(&cid, "entry.view")
        })
        .collect();

    let grows: Vec<GRow> = sqlx::query_as(
        "SELECT g.id AS id, g.cluster_id AS cluster_id, g.parent_id AS parent_id, g.name AS name,
                (SELECT COUNT(*) FROM entries e WHERE e.group_id = g.id) AS cnt
         FROM groups g",
    )
    .fetch_all(&state.db)
    .await?;

    let mut by_cluster: std::collections::HashMap<String, Vec<GRow>> = Default::default();
    for g in grows {
        by_cluster.entry(g.cluster_id.clone()).or_default().push(g);
    }

    let mut out = Vec::new();
    for c in c_rows {
        let cid: String = c.get("id");
        let rows = by_cluster.remove(&cid).unwrap_or_default();

        // id -> düğüm haritası + üstten çocuk bağlantısı
        let mut nodes: std::collections::HashMap<String, GroupNode> = Default::default();
        let mut children: std::collections::HashMap<String, Vec<String>> = Default::default();
        for g in &rows {
            nodes.insert(
                g.id.clone(),
                GroupNode {
                    id: g.id.clone(),
                    cluster_id: cid.clone(),
                    parent_id: g.parent_id.clone(),
                    name: g.name.clone(),
                    entry_count: g.cnt,
                    children: Vec::new(),
                },
            );
            if let Some(pid) = &g.parent_id {
                if nodes.contains_key(pid) {
                    children.entry(pid.clone()).or_default().push(g.id.clone());
                }
            }
        }

        // kökler: parent'ı yok ya da aynı klusterde örneğine rastlanmadı (bozuk kayıt toleransı)
        // attach: düğümü haritadan söker, çocuklarını özyinelemeli bağlar,
        // alt ağaç toplamını hesaplar (direkt + çocukların toplamı), çocukları ada göre sıralar.
        fn attach(
            id: &str,
            nodes: &mut std::collections::HashMap<String, GroupNode>,
            children: &std::collections::HashMap<String, Vec<String>>,
        ) -> GroupNode {
            let mut node = nodes.remove(id).expect("id haritada mevcut");
            let child_ids = children.get(id).cloned().unwrap_or_default();
            for ch in &child_ids {
                node.children.push(attach(ch, nodes, children));
            }
            node.children.sort_by(|a, b| a.name.cmp(&b.name));
            let sub: i64 = node.children.iter().map(|c| c.entry_count).sum();
            node.entry_count += sub;
            node
        }
        let mut root_ids: Vec<String> = rows
            .iter()
            .filter(|g| {
                g.parent_id
                    .as_ref()
                    .map(|p| !nodes.contains_key(p))
                    .unwrap_or(true)
            })
            .map(|g| g.id.clone())
            .collect();
        root_ids.sort();

        let mut cluster_total = 0i64;
        let mut root_nodes: Vec<GroupNode> = Vec::new();
        for r in &root_ids {
            let node = attach(r, &mut nodes, &children);
            cluster_total += node.entry_count;
            root_nodes.push(node);
        }
        root_nodes.sort_by(|a, b| a.name.cmp(&b.name));

        out.push(NavCluster {
            id: cid,
            name: c.get("name"),
            slug: c.get("slug"),
            color: c.get("color"),
            entry_count: cluster_total,
            roots: root_nodes,
        });
    }
    Ok(Json(out))
}
