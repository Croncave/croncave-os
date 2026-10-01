//! Data the platform starts with: the first plan catalog version, the Croncave color
//! scheme, and the built-in watcher types. Seeding never overwrites existing versions.

use serde_json::Value;
use sqlx::PgPool;

pub async fn run(db: &PgPool) -> anyhow::Result<()> {
    let (n,): (i64,) = sqlx::query_as("select count(*) from catalog_versions").fetch_one(db).await?;
    if n == 0 {
        let c = crate::catalog::seed();
        sqlx::query("insert into catalog_versions (starts_at, data, note) values (now() - interval '1 day', $1, 'Starting figures from the product definition')")
            .bind(serde_json::to_value(&c)?)
            .execute(db)
            .await?;
    }
    let scheme: Value = serde_json::from_str(include_str!("../catalog/scheme-croncave.json"))?;
    sqlx::query("insert into color_schemes (id, version, data) values ($1, $2, $3) on conflict do nothing")
        .bind(scheme["id"].as_str().unwrap_or("croncave"))
        .bind(scheme["version"].as_i64().unwrap_or(1) as i32)
        .bind(&scheme)
        .execute(db)
        .await?;
    for t in crate::watcher::builtin_types() {
        sqlx::query("insert into watcher_types (id, version, config, origin, approved_at) values ($1, $2, $3, 'builtin', now()) on conflict do nothing")
            .bind(&t.id)
            .bind(t.version as i32)
            .bind(serde_json::to_value(&t)?)
            .execute(db)
            .await?;
    }
    Ok(())
}
