//! Disposable `workers-rs` comparison host.
//!
//! This crate intentionally does not replace `apps/workers`. It proves that
//! the official Rust SDK can expose the existing Directory and Web Plugins
//! against real local D1/R2 bindings. Admission, cancellation fencing,
//! generation retirement and cleanup remain owned by the production host until
//! a separate lifecycle qualification passes.

#[cfg(target_arch = "wasm32")]
mod wasm {
    use anyhow::{Context as _, Result, anyhow, bail};
    use bytes::Bytes;
    use futures::future::LocalBoxFuture;
    use lenso_app_plan::authoring::{
        HostBinding, HostCatalog, HostDefaultPlugin, HostPluginRelease, HostSlot, PluginInstanceId,
        PluginRootSnapshot, resolve_plugin_root,
    };
    use lenso_kernel::{CancellationToken, Kernel, RuntimeFailure, ShutdownOutcome};
    use lenso_marketplace_directory_plugin::EventDirectoryFactory;
    use lenso_marketplace_web_plugin::cache_storage::{AcceptedEnvelope, CacheStorage};
    use lenso_marketplace_web_plugin::{EventWebFactory, MarketplaceClock};
    use lenso_native_adapter::NativePluginRegistry;
    use lenso_web_ingress_plugin::{WebIngressConfig, WebIngressEventFactory};
    use lenso_workers_driver::WorkersDriver;
    use serde::Deserialize;
    use std::{cell::RefCell, fmt::Write as _, rc::Rc, time::Duration};
    use worker::{
        Bucket, Conditional, Context, Env, Error as WorkerError, Headers, Request, Response,
        Result as WorkerResult,
        d1::{D1Database, D1PreparedStatement, D1Type},
        event,
    };

    const MAX_ENVELOPE: u64 = 4 * 1024 * 1024;
    const MAX_STATE: u64 = 12 * 1024 * 1024;
    const STORAGE_BINDING: &str = "MARKETPLACE";

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct PublicationPointer {
        object_key: String,
        digest: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct AcceptedPointer {
        token: String,
        object_key: String,
    }

    #[derive(Debug)]
    struct CatalogRead {
        envelope: Option<String>,
        accepted: Option<AcceptedEnvelope>,
    }

    #[derive(Debug)]
    struct Storage {
        database: D1Database,
        bucket: Bucket,
        // The direct Rust host keeps the same one-shot handoff as the
        // production adapter. This is state for one event only.
        pending_catalog: RefCell<Option<String>>,
        prior: RefCell<Option<(String, Option<AcceptedEnvelope>)>>,
    }

    impl Storage {
        fn new(database: D1Database, bucket: Bucket) -> Self {
            Self {
                database,
                bucket,
                pending_catalog: RefCell::default(),
                prior: RefCell::default(),
            }
        }

        async fn object_bytes(
            &self,
            key: &str,
            limit: u64,
            existing: Option<worker::Object>,
        ) -> Result<Vec<u8>> {
            let object = match existing {
                Some(object) => Some(object),
                None => self
                    .bucket
                    .get(key)
                    .execute()
                    .await
                    .map_err(|error| anyhow!("{error:?}"))?,
            }
            .ok_or_else(|| anyhow!("catalog object missing"))?;
            if object.size() > limit {
                bail!("catalog object exceeds limit");
            }
            let body = object
                .body()
                .ok_or_else(|| anyhow!("catalog object body missing"))?;
            let bytes = body.bytes().await.map_err(|error| anyhow!("{error:?}"))?;
            if bytes.len() as u64 > limit {
                bail!("catalog object exceeds limit");
            }
            Ok(bytes)
        }

        fn publication_statement(&self, catalog: &str) -> Result<D1PreparedStatement> {
            let parameter = D1Type::Text(catalog);
            self.database
                .prepare(
                    "SELECT object_key,digest FROM marketplace_publications WHERE catalog_id=?",
                )
                .bind_refs(&parameter)
                .map_err(|error| anyhow!("{error:?}"))
        }

        fn accepted_statement(&self, catalog: &str) -> Result<D1PreparedStatement> {
            let parameter = D1Type::Text(catalog);
            self.database
                .prepare("SELECT token,object_key FROM marketplace_accepted WHERE catalog_id=?")
                .bind_refs(&parameter)
                .map_err(|error| anyhow!("{error:?}"))
        }

        async fn publication_pointer(&self, catalog: &str) -> Result<Option<PublicationPointer>> {
            self.publication_statement(catalog)?
                .first(None)
                .await
                .map_err(|error| anyhow!("{error:?}"))
        }

        async fn accepted_pointer(&self, catalog: &str) -> Result<Option<AcceptedPointer>> {
            self.accepted_statement(catalog)?
                .first(None)
                .await
                .map_err(|error| anyhow!("{error:?}"))
        }

        async fn published(&self, catalog: &str) -> Result<Option<String>> {
            let pointer = self.publication_pointer(catalog).await?;
            let Some(pointer) = pointer else {
                return Ok(None);
            };
            Ok(Some(self.published_object(&pointer).await?))
        }

        async fn published_object(&self, pointer: &PublicationPointer) -> Result<String> {
            if pointer.object_key.is_empty() || !is_digest(&pointer.digest) {
                bail!("invalid publication pointer");
            }
            let bytes = self
                .object_bytes(&pointer.object_key, MAX_ENVELOPE, None)
                .await?;
            if lenso_plugin_catalog::digest(&bytes) != pointer.digest {
                bail!("published object integrity failure");
            }
            String::from_utf8(bytes).context("published object is not UTF-8")
        }

        async fn accepted(&self, catalog: &str) -> Result<Option<AcceptedEnvelope>> {
            let pointer = self.accepted_pointer(catalog).await?;
            let Some(pointer) = pointer else {
                return Ok(None);
            };
            Ok(Some(self.accepted_object(catalog, &pointer).await?))
        }

        async fn accepted_object(
            &self,
            catalog: &str,
            pointer: &AcceptedPointer,
        ) -> Result<AcceptedEnvelope> {
            let raw = self
                .object_bytes(&pointer.object_key, MAX_STATE, None)
                .await?;
            let raw_digest = lenso_plugin_catalog::digest(&raw);
            let expected_key = accepted_key(catalog, &raw_digest)?;
            if pointer.object_key != expected_key {
                bail!("accepted checkpoint integrity failure");
            }
            let state: AcceptedEnvelope =
                serde_json::from_slice(&raw).context("invalid accepted checkpoint")?;
            if state.token != pointer.token
                || lenso_plugin_catalog::digest(&state.envelope) != state.token
            {
                bail!("accepted pointer mismatch");
            }
            Ok(state)
        }

        async fn catalog(&self, catalog: &str) -> Result<CatalogRead> {
            // The two pointer reads stay in one primary D1 batch, matching the
            // production adapter's consistency boundary.
            let publication = self.publication_statement(catalog)?;
            let accepted = self.accepted_statement(catalog)?;
            let rows = self
                .database
                .batch(vec![publication, accepted])
                .await
                .map_err(|error| anyhow!("{error:?}"))?;
            if rows.len() != 2 || rows.iter().any(|row| !row.success()) {
                bail!("catalog pointer read failed");
            }

            let publications: Vec<PublicationPointer> =
                rows[0].results().map_err(|error| anyhow!("{error:?}"))?;
            let accepted_pointers: Vec<AcceptedPointer> =
                rows[1].results().map_err(|error| anyhow!("{error:?}"))?;
            if publications.len() > 1 || accepted_pointers.len() > 1 {
                bail!("catalog pointer read returned duplicate rows");
            }

            let publication = publications.into_iter().next();
            if let Some(pointer) = &publication
                && (pointer.object_key.is_empty() || !is_digest(&pointer.digest))
            {
                bail!("invalid publication pointer");
            }
            let accepted = match accepted_pointers.into_iter().next() {
                Some(pointer) => Some(self.accepted_object(catalog, &pointer).await?),
                None => None,
            };
            let envelope = match (&publication, &accepted) {
                (Some(publication), Some(accepted)) if publication.digest == accepted.token => {
                    Some(
                        String::from_utf8(accepted.envelope.clone())
                            .context("accepted envelope is not UTF-8")?,
                    )
                }
                (Some(publication), _) => Some(self.published_object(publication).await?),
                (None, _) => None,
            };
            Ok(CatalogRead { envelope, accepted })
        }

        async fn compare_exchange(
            &self,
            catalog: &str,
            expected: Option<&str>,
            value: &AcceptedEnvelope,
        ) -> Result<bool> {
            if lenso_plugin_catalog::digest(&value.envelope) != value.token {
                bail!("invalid accepted content token");
            }
            let serialized = serde_json::to_vec(value)?;
            if serialized.len() as u64 > MAX_STATE {
                bail!("accepted state exceeds bound");
            }
            let stored_digest = lenso_plugin_catalog::digest(&serialized);
            let key = accepted_key(catalog, &stored_digest)?;
            let expected_key = match expected {
                Some(expected) => {
                    let pointer = self.accepted_pointer(catalog).await?;
                    let Some(pointer) = pointer else {
                        return Ok(false);
                    };
                    if pointer.token != expected {
                        return Ok(false);
                    }
                    if pointer.object_key.is_empty() {
                        bail!("invalid accepted pointer");
                    }
                    Some(pointer.object_key)
                }
                None => None,
            };

            let existing = self
                .bucket
                .get(&key)
                .execute()
                .await
                .map_err(|error| anyhow!("{error:?}"))?;
            if let Some(existing) = existing {
                if self.object_bytes(&key, MAX_STATE, Some(existing)).await? != serialized {
                    bail!("immutable accepted object conflict");
                }
            } else {
                let created = self
                    .bucket
                    .put(&key, serialized.clone())
                    .only_if(Conditional {
                        etag_does_not_match: Some("*".into()),
                        ..Conditional::default()
                    })
                    .execute()
                    .await
                    .map_err(|error| anyhow!("{error:?}"))?;
                if created.is_none()
                    && self.object_bytes(&key, MAX_STATE, None).await? != serialized
                {
                    bail!("immutable accepted object conflict");
                }
            }

            let receipt_result = match expected {
                Some(expected) => {
                    let catalog_parameter = D1Type::Text(catalog);
                    let value_token = D1Type::Text(&value.token);
                    let value_key = D1Type::Text(&key);
                    let expected_token = D1Type::Text(expected);
                    let expected_key_parameter = D1Type::Text(
                        expected_key
                            .as_deref()
                            .ok_or_else(|| anyhow!("accepted pointer key missing"))?,
                    );
                    self.database
                        .prepare(
                            "UPDATE marketplace_accepted SET token=?,object_key=? \
                             WHERE catalog_id=? AND token=? AND object_key=?",
                        )
                        .bind_refs([
                            &value_token,
                            &value_key,
                            &catalog_parameter,
                            &expected_token,
                            &expected_key_parameter,
                        ])
                        .map_err(|error| anyhow!("{error:?}"))?
                        .run()
                        .await
                }
                None => {
                    let catalog_parameter = D1Type::Text(catalog);
                    let value_token = D1Type::Text(&value.token);
                    let value_key = D1Type::Text(&key);
                    self.database
                        .prepare(
                            "INSERT INTO marketplace_accepted(catalog_id,token,object_key) \
                             VALUES(?,?,?) ON CONFLICT(catalog_id) DO NOTHING",
                        )
                        .bind_refs([&catalog_parameter, &value_token, &value_key])
                        .map_err(|error| anyhow!("{error:?}"))?
                        .run()
                        .await
                }
            };
            let receipt = match receipt_result {
                Ok(receipt) if receipt.success() => receipt,
                Ok(_) => {
                    return self
                        .reconcile_pointer(
                            catalog,
                            &value.token,
                            &key,
                            "D1 accepted pointer write was not successful",
                        )
                        .await;
                }
                Err(error) => {
                    return self
                        .reconcile_pointer(catalog, &value.token, &key, error)
                        .await;
                }
            };
            Ok(receipt
                .meta()
                .map_err(|error| anyhow!("{error:?}"))?
                .and_then(|meta| meta.changes)
                == Some(1))
        }

        async fn reconcile_pointer(
            &self,
            catalog: &str,
            token: &str,
            object_key: &str,
            reason: impl std::fmt::Debug,
        ) -> Result<bool> {
            let pointer = self.accepted_pointer(catalog).await.map_err(|error| {
                anyhow!("accepted pointer write uncertain ({reason:?}); reconciliation failed: {error:?}")
            })?;
            match pointer {
                Some(pointer) if pointer.token == token && pointer.object_key == object_key => {
                    Ok(true)
                }
                Some(_) => Ok(false),
                None => bail!("accepted pointer write uncertain: {reason:?}"),
            }
        }
    }

    impl lenso_marketplace_directory_plugin::storage::PublishedStorage for Storage {
        fn published<'a>(
            &'a self,
            catalog: &'a str,
        ) -> LocalBoxFuture<'a, anyhow::Result<Option<String>>> {
            let coalesce = self.pending_catalog.borrow_mut().take().as_deref() == Some(catalog);
            self.prior.borrow_mut().take();
            Box::pin(async move {
                if coalesce {
                    let read = self.catalog(catalog).await?;
                    self.prior
                        .replace(Some((catalog.to_owned(), read.accepted)));
                    Ok(read.envelope)
                } else {
                    self.published(catalog).await
                }
            })
        }
    }

    impl CacheStorage for Storage {
        fn prepare_catalog_read(&self, catalog: &str) {
            self.prior.borrow_mut().take();
            self.pending_catalog.replace(Some(catalog.to_owned()));
        }

        fn accepted<'a>(
            &'a self,
            catalog: &'a str,
        ) -> LocalBoxFuture<'a, anyhow::Result<Option<AcceptedEnvelope>>> {
            let prior = self.prior.borrow_mut().take();
            Box::pin(async move {
                if let Some((owner, accepted)) = prior
                    && owner == catalog
                {
                    return Ok(accepted);
                }
                self.accepted(catalog).await
            })
        }

        fn compare_exchange<'a>(
            &'a self,
            catalog: &'a str,
            expected: Option<&'a str>,
            value: &'a AcceptedEnvelope,
        ) -> LocalBoxFuture<'a, anyhow::Result<bool>> {
            Box::pin(self.compare_exchange(catalog, expected, value))
        }
    }

    #[derive(Debug)]
    struct Clock;

    impl MarketplaceClock for Clock {
        fn now(&self) -> Result<u64, RuntimeFailure> {
            let now = js_sys::Date::now();
            if !now.is_finite() || now < 0.0 {
                return Err(RuntimeFailure::PluginFailure {
                    detail: "Workers SDK prototype clock failed".into(),
                });
            }
            Ok((now / 1000.0).floor() as u64)
        }
    }

    #[derive(Debug)]
    struct DriverGuard(WorkersDriver);

    impl Drop for DriverGuard {
        fn drop(&mut self) {
            self.0.request_shutdown();
        }
    }

    fn worker_error(error: impl std::fmt::Debug) -> WorkerError {
        WorkerError::RustError(format!("{error:?}"))
    }

    fn is_digest(value: &str) -> bool {
        let Some(hex) = value.strip_prefix("sha256:") else {
            return false;
        };
        hex.len() == 64
            && hex
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    }

    fn encode_component(value: &str) -> String {
        let mut encoded = String::with_capacity(value.len());
        for byte in value.bytes() {
            if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
                encoded.push(byte as char);
            } else {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
        encoded
    }

    fn accepted_key(catalog: &str, digest: &str) -> Result<String> {
        let digest = digest
            .strip_prefix("sha256:")
            .ok_or_else(|| anyhow!("invalid accepted state digest"))?;
        Ok(format!(
            "accepted/{}/{digest}.json",
            encode_component(catalog)
        ))
    }

    #[derive(Debug)]
    struct Configuration {
        catalog_id: String,
        key_id: String,
        public_key_hex: String,
    }

    impl Configuration {
        fn from_env(env: &Env) -> WorkerResult<Self> {
            Ok(Self {
                catalog_id: env.var("CATALOG_ID")?.to_string(),
                key_id: env.var("CATALOG_KEY_ID")?.to_string(),
                public_key_hex: env.var("CATALOG_PUBLIC_KEY")?.to_string(),
            })
        }
    }

    fn resolve_plan(
        configuration: &Configuration,
    ) -> WorkerResult<lenso_app_plan::ResolvedAppPlan> {
        let releases = vec![
            HostPluginRelease::new(WebIngressEventFactory::plugin_descriptor()),
            HostPluginRelease::new(
                serde_json::from_str(lenso_marketplace_directory_plugin::PLUGIN_DESCRIPTOR_JSON)
                    .map_err(worker_error)?,
            ),
            HostPluginRelease::new(
                serde_json::from_str(lenso_marketplace_web_plugin::PLUGIN_DESCRIPTOR_JSON)
                    .map_err(worker_error)?,
            ),
        ];
        let defaults = [
            HostDefaultPlugin::new("lenso.web-ingress", "default").with_configuration(
                serde_json::to_value(WebIngressConfig::default()).map_err(worker_error)?,
            ),
            HostDefaultPlugin::new("lenso.marketplace.directory", "default").with_configuration(
                serde_json::json!({
                    "catalog_id": configuration.catalog_id,
                    "storage_binding": STORAGE_BINDING
                }),
            ),
            HostDefaultPlugin::new("lenso.marketplace.web", "default").with_configuration(
                serde_json::json!({
                    "catalog_id": configuration.catalog_id,
                    "key_id": configuration.key_id,
                    "public_key_hex": configuration.public_key_hex,
                    "storage_binding": STORAGE_BINDING
                }),
            ),
        ];
        let host = HostCatalog::new(
            [
                HostSlot::one("http-ingress"),
                HostSlot::one("marketplace-directories"),
                HostSlot::one("web"),
            ],
            releases,
            defaults,
        )
        .with_bindings([
            HostBinding::new(
                PluginInstanceId::new("lenso.web-ingress", "default"),
                lenso_capability_http_endpoint::CAPABILITY_ID,
                "web",
            ),
            HostBinding::new(
                PluginInstanceId::new("lenso.marketplace.web", "default"),
                lenso_capability_marketplace_directory::CAPABILITY_ID,
                "marketplace-directories",
            ),
        ]);
        resolve_plugin_root(&host, &PluginRootSnapshot::default())
            .map(|resolved| resolved.plan().clone())
            .map_err(worker_error)
    }

    async fn to_http_request(mut request: Request) -> WorkerResult<http::Request<Bytes>> {
        let url = request.url()?;
        let mut uri = url.path().to_owned();
        if let Some(query) = url.query() {
            uri.push('?');
            uri.push_str(query);
        }
        let method =
            http::Method::from_bytes(request.method().as_ref().as_bytes()).map_err(worker_error)?;
        let mut builder = http::Request::builder().method(method).uri(uri);
        for (name, value) in request.headers() {
            builder = builder.header(name, value);
        }
        builder
            .body(Bytes::from(request.bytes().await?))
            .map_err(worker_error)
    }

    fn to_worker_response(response: http::Response<Bytes>) -> WorkerResult<Response> {
        let (parts, body) = response.into_parts();
        let headers = Headers::from(&parts.headers);
        Response::builder()
            .with_status(parts.status.as_u16())
            .with_headers(headers)
            .from_bytes(body.to_vec())
    }

    #[event(fetch)]
    pub async fn main(request: Request, env: Env, _context: Context) -> WorkerResult<Response> {
        let configuration = Configuration::from_env(&env)?;
        let storage = Rc::new(Storage::new(
            env.d1("MARKETPLACE_DB")?,
            env.bucket("MARKETPLACE_OBJECTS")?,
        ));

        lenso_marketplace_directory_plugin::link_plugin();
        lenso_marketplace_web_plugin::link_plugin();
        let ingress = WebIngressEventFactory::new();
        let registry = NativePluginRegistry::new()
            .with_factory(ingress.clone())
            .with_factory_override(EventDirectoryFactory::new(
                STORAGE_BINDING.into(),
                storage.clone(),
            ))
            .map_err(worker_error)?
            .with_factory_override(EventWebFactory::new(
                STORAGE_BINDING.into(),
                storage,
                Rc::new(Clock),
            ))
            .map_err(worker_error)?
            .with_linked_factories();
        let plan = resolve_plan(&configuration)?;
        let driver = WorkersDriver::new();
        let _guard = DriverGuard(driver.clone());
        let app = Kernel::start_native(plan, driver, registry)
            .await
            .map_err(worker_error)?;
        let request = to_http_request(request).await?;
        let cancellation = CancellationToken::new();
        let outcome = ingress
            .handle(request, cancellation)
            .await
            .map_err(worker_error);
        let shutdown = app.shutdown(Duration::from_secs(1)).await;
        if !matches!(shutdown, ShutdownOutcome::Clean) {
            return Err(worker_error("Workers SDK prototype shutdown was not clean"));
        }
        to_worker_response(outcome?)
    }
}
