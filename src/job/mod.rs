use crate::infra::cluster;
use crate::infra::config::CONFIG;
use crate::meta::user::User;
use crate::service::{db, users};

mod alert_manager;
mod compact;
mod file_list;
mod files;
mod prom;

pub async fn init() -> Result<(), anyhow::Error> {
    let res = db::user::get_root_user(&CONFIG.auth.username).await;
    if res.is_err() || res.unwrap().is_none() {
        let _ = users::post_user(
            "dummy",
            User {
                name: CONFIG.auth.username.clone(),
                password: CONFIG.auth.password.clone(),
                role: crate::meta::user::UserRole::Root,
                salt: String::new(),
            },
        )
        .await;
    }
    tokio::task::spawn(async move { db::functions::watch().await });
    tokio::task::spawn(async move { db::user::watch().await });
    tokio::task::spawn(async move { db::schema::watch().await });
    tokio::task::spawn(async move { db::watch_prom_cluster_leader().await });
    tokio::task::spawn(async move { db::alerts::watch().await });
    tokio::task::spawn(async move { db::triggers::watch().await });
    tokio::task::yield_now().await; // yield let other tasks run
    db::functions::cache().await?;
    db::user::cache().await?;
    db::schema::cache().await?;
    db::cache_prom_cluster_leader().await?;
    db::alerts::cache().await?;
    db::triggers::cache().await?;

    // cache file list
    db::file_list::local::cache().await?;
    db::file_list::remote::cache().await?;

    // Shouldn't serve request until initialization finishes
    log::info!("[TRACE] Start job");

    // compactor run
    tokio::task::spawn(async move { compact::run().await });

    // alert manager run
    tokio::task::spawn(async move { alert_manager::run().await });

    // ingester run
    tokio::task::spawn(async move { files::run().await });
    tokio::task::spawn(async move { file_list::run().await });
    tokio::task::spawn(async move { prom::run().await });

    Ok(())
}

#[cfg(test)]
mod test_utils {
    use super::*;
    use std::env;
    #[actix_web::test]
    async fn test_init() {
        env::set_var("ZIOX_LOCAL_MODE", "true");
        env::set_var("ZIOX_NODE_ROLE", "all");
        let _ = init().await;
        //assert_eq!(fs::metadata(&CONFIG.common.data_wal_dir).is_ok(), true)
    }
}