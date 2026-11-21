//! Geliştirme DB'inde sqlx migration checksum'larını migration dosyalarıyla
//! karşılaştırır; `--apply` verilirse DB'deki checksum'ı dosyanın checksum'ına
//! günceller (dosya uygulandıktan sonra kozmetik olarak düzenlendiyse).
//!
//! Kullanım: `cargo run -p vardrop-server --bin dbfix -- <db-yolu> [--apply]`

use anyhow::{bail, Context, Result};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::Connection;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        bail!("kullanım: dbfix <db-yolu> [--apply]");
    }
    let db_path = args[1].clone();
    let apply = args.iter().skip(2).any(|a| a == "--apply");

    // Derleme zamanında gömülü migration checksum'ları (SHA-384).
    let migrator = sqlx::migrate!("./migrations");
    let files: Vec<(i64, Vec<u8>)> = migrator
        .iter()
        .map(|m| (m.version, m.checksum.to_vec()))
        .collect();

    let url = format!("sqlite://{db_path}");
    let opts = SqliteConnectOptions::from_str(&url)?.read_only(!apply);
    let mut conn = sqlx::sqlite::SqliteConnection::connect_with(&opts).await?;

    let rows: Vec<(i64, String, Vec<u8>)> = sqlx::query_as(
        "SELECT version, description, checksum FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(&mut conn)
    .await?;

    if rows.is_empty() {
        bail!("_sqlx_migrations boş — DB daha hiç migrate edilmemiş");
    }

    let mut mismatched = 0;
    for (version, description, applied) in rows {
        let Some((_, file)) = files.iter().find(|(v, _)| *v == version) else {
            println!("v{version} ({description}): uyarı — dosyası yok (migration silinmiş?)");
            continue;
        };
        if applied == *file {
            println!("v{version} ({description}): OK");
        } else {
            mismatched += 1;
            println!(
                "v{version} ({description}): CHECKSUM FARKLI\n  db:   {}\n  file: {}",
                hex::encode(&applied),
                hex::encode(file)
            );
            if apply {
                let res = sqlx::query("UPDATE _sqlx_migrations SET checksum = ? WHERE version = ?")
                    .bind(file)
                    .bind(version)
                    .execute(&mut conn)
                    .await
                    .context("checksum güncellenemedi")?;
                if res.rows_affected() == 1 {
                    println!("  -> checksum güncellendi");
                }
            }
        }
    }
    conn.close().await?;

    if apply {
        println!("\ntamam — şimdi `cargo run` migration'ları doğrulayıp eksikleri uygular");
    } else if mismatched > 0 {
        println!("\n{mismatched} migration değişmiş — `--apply` ile düzelt");
    }
    Ok(())
}
