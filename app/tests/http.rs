use lenso_marketplace_catalog::directory::Directory;
use std::{
    collections::BTreeSet,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[tokio::test(flavor = "current_thread")]
async fn real_host_serves_verified_catalog_and_honest_failures() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let root = tempfile::tempdir().unwrap();
            let database = root.path().join("directory.sqlite3");
            let mut publisher =
                Directory::open(&database, "test", BTreeSet::from(["reviewer".into()])).unwrap();
            let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
            let config = lenso_marketplace_app::Config {
                app_root: root.path().join("app"),
                directory_database: database,
                catalog_id: "test".into(),
                key_id: "test-key".into(),
                public_key_hex: key
                    .verifying_key()
                    .to_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                address: "127.0.0.1:0".parse().unwrap(),
            };
            let (app, address) = lenso_marketplace_app::start(&config).await.unwrap();
            let client = reqwest::Client::new();
            // Marketplace must not expose Agent discovery or mutation proxies.
            assert_eq!(
                client
                    .get(format!("http://{address}/api/marketplace/v1/targets"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                404
            );
            for path in ["connect", "proposals", "operations", "operations/test"] {
                assert_eq!(
                    client
                        .post(format!(
                            "http://{address}/api/marketplace/v1/targets/local/{path}"
                        ))
                        .send()
                        .await
                        .unwrap()
                        .status(),
                    404
                );
            }
            // Transport-owned headers on assets used to produce a 502 and a blank page.
            for (path, content_type) in [
                ("/", "text/html"),
                ("/marketplace.js", "text/javascript"),
                ("/marketplace.css", "text/css"),
            ] {
                let response = client
                    .get(format!("http://{address}{path}"))
                    .send()
                    .await
                    .unwrap();
                assert_eq!(response.status(), 200);
                assert!(
                    response.headers()["content-type"]
                        .to_str()
                        .unwrap()
                        .starts_with(content_type)
                );
                assert_eq!(response.headers()["x-content-type-options"], "nosniff");
            }
            let base = format!("http://{address}/api/marketplace/v1/plugins");
            assert!(
                client
                    .get(&base)
                    .send()
                    .await
                    .unwrap()
                    .status()
                    .is_server_error()
            );
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            publisher
                .publish("reviewer", 0, now, now + 3600, "test-key", &key)
                .unwrap();
            let response = client.get(&base).send().await.unwrap();
            let status = response.status();
            let body: serde_json::Value = response.json().await.unwrap();
            assert_eq!(status, 200, "{body}");
            assert_eq!(body["catalog_id"], "test");
            assert_eq!(body["releases"], serde_json::json!([]));
            // Agent consumers need the original signed envelope, not display JSON.
            let envelope = client
                .get(format!("http://{address}/api/marketplace/v1/snapshot"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let trust = lenso_marketplace_catalog::Trust {
                catalog_id: "test".into(),
                keys: std::collections::BTreeMap::from([("test-key".into(), key.verifying_key())]),
            };
            assert_eq!(
                lenso_marketplace_catalog::verify(envelope.as_bytes(), &trust, None, now)
                    .unwrap()
                    .snapshot()
                    .revision,
                1
            );

            assert_eq!(
                client
                    .get(format!("{base}?limit=101"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                400
            );
            assert_eq!(
                client
                    .get(format!("{base}/example.missing/1.0.0"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                404
            );
            // Cache-only tests cannot catch a verification failure killing a live HTTP
            // provider, or HTTP silently falling back after an invalid new snapshot.
            let wrong_key = ed25519_dalek::SigningKey::from_bytes(&[18; 32]);
            publisher
                .publish("reviewer", 1, now, now + 3600, "test-key", &wrong_key)
                .unwrap();
            assert_eq!(client.get(&base).send().await.unwrap().status(), 503);
            publisher
                .publish("reviewer", 2, now, now + 3600, "test-key", &key)
                .unwrap();
            let recovered = client.get(&base).send().await.unwrap();
            assert_eq!(recovered.status(), 200);
            let recovered: serde_json::Value = recovered.json().await.unwrap();
            assert_eq!(recovered["revision"], 3);
            assert_eq!(recovered["cached"], false);
            // Expiry must not turn display metadata into an HTTP outage or an
            // installation grant. Use an already-expired signed publication.
            publisher
                .publish("reviewer", 3, now - 100, now - 50, "test-key", &key)
                .unwrap();
            let expired = client.get(&base).send().await.unwrap();
            assert_eq!(expired.status(), 200);
            let expired: serde_json::Value = expired.json().await.unwrap();
            assert_eq!(expired["stale"], true);
            assert_eq!(expired["expires_at"], now - 50);
            let envelope = client
                .get(format!("http://{address}/api/marketplace/v1/snapshot"))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            assert!(
                lenso_marketplace_catalog::verify(envelope.as_bytes(), &trust, None, now).is_err()
            );
            assert!(matches!(
                app.shutdown(Duration::from_secs(5)).await,
                lenso_kernel::ShutdownOutcome::Clean
            ));
            // Existing Plugin Root disablement removes the HTTP contribution on restart.
            lenso_app_authoring::set_instance_disabled(
                &config.app_root,
                "lenso.marketplace.web",
                "default",
                true,
            )
            .unwrap();
            // This dedicated Host has no routes when its only Web provider is removed.
            // Web Ingress correctly rejects an empty route table instead of retaining stale routes.
            let error = match lenso_marketplace_app::start(&config).await {
                Ok(_) => panic!("disabled Web provider must not leave routes behind"),
                Err(error) => error,
            };
            assert!(
                error
                    .to_string()
                    .contains("at least one bound HTTP Endpoint route")
            );
        })
        .await;
}
