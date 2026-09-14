//! Marketplace HTTP presentation. All directory reads are Plan-bound.
use lenso::prelude::*;
use lenso_capability_http_endpoint::{
    self as http_endpoint_contract, EndpointHandleInvocationError, HandleResponse, Path, Query,
    endpoint,
    response::{self, StatusCode},
};
use lenso_capability_marketplace_directory as directory;
use lenso_kernel::{DeactivateContext, InvocationContext, PrepareContext, RuntimeFailure};
use lenso_marketplace_catalog::{Trust, VerifiedSnapshot, cache::VerifiedCache};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::PathBuf,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    catalog_id: String,
    key_id: String,
    public_key_hex: String,
    cache_database: PathBuf,
}

#[lenso::plugin(lifecycle, configuration_schema = "config.schema.json")]
#[derive(Clone, Debug)]
struct MarketplaceWeb {
    #[config]
    config: Config,
    directory: Port<directory::DirectoryClient>,
    cache: Rc<RefCell<Option<VerifiedCache>>>,
    #[tasks]
    tasks: lenso::ManagedTasks,
}
impl lenso::Lifecycle for MarketplaceWeb {
    async fn prepare(&self, _context: PrepareContext) -> Result<(), RuntimeFailure> {
        let bytes: [u8; 32] = hex::decode(&self.config.public_key_hex)
            .map_err(failure)?
            .try_into()
            .map_err(|_| failure("invalid public key length"))?;
        let trust = Trust {
            catalog_id: self.config.catalog_id.clone(),
            keys: BTreeMap::from([(
                self.config.key_id.clone(),
                ed25519_dalek::VerifyingKey::from_bytes(&bytes).map_err(failure)?,
            )]),
        };
        self.cache.replace(Some(
            VerifiedCache::open(&self.config.cache_database, trust).map_err(failure)?,
        ));
        Ok(())
    }
    async fn deactivate(&self, _context: DeactivateContext) -> Result<(), RuntimeFailure> {
        self.cache.take();
        Ok(())
    }
}
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Search {
    #[serde(default)]
    q: String,
    publisher: Option<String>,
    license: Option<String>,
    ids: Option<String>,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize {
    30
}
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleasePath {
    plugin_id: String,
    version: String,
}

#[derive(Debug, serde::Deserialize)]
struct SampleAssetPath {
    name: String,
}

impl MarketplaceWeb {
    async fn catalog(
        &self,
        context: InvocationContext,
    ) -> Result<(VerifiedSnapshot, bool), RuntimeFailure> {
        let result = self
            .directory
            .read_snapshot_with_context(context, directory::ReadSnapshotRequest {})
            .await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(failure)?
            .as_secs();
        let mut state = self.cache.borrow_mut();
        let cache = state
            .as_mut()
            .ok_or_else(|| failure("marketplace Web is unavailable"))?;
        match result {
            Ok(response) => Ok((
                cache
                    .accept(response.envelope_json.as_str().as_bytes(), now)
                    .map_err(failure)?,
                false,
            )),
            Err(directory::DirectoryInvocationError::Runtime(_)) => cache
                .current(now)
                .map_err(failure)?
                .map(|snapshot| (snapshot, true))
                .ok_or_else(|| {
                    failure("directory unavailable and no current verified cache exists")
                }),
            Err(error) => Err(failure(format!("{error:?}"))),
        }
    }
}

#[endpoint]
impl MarketplaceWeb {
    #[get("marketplace.index", "/")]
    async fn index(&self) -> Result<HandleResponse, EndpointHandleInvocationError> {
        Ok(asset(
            "text/html; charset=utf-8",
            include_bytes!("../ui/dist/index.html"),
        ))
    }
    #[get("marketplace.script", "/marketplace.js")]
    async fn script(&self) -> Result<HandleResponse, EndpointHandleInvocationError> {
        Ok(asset(
            "text/javascript; charset=utf-8",
            include_bytes!("../ui/dist/marketplace.js"),
        ))
    }
    #[get("marketplace.styles", "/marketplace.css")]
    async fn styles(&self) -> Result<HandleResponse, EndpointHandleInvocationError> {
        Ok(asset(
            "text/css; charset=utf-8",
            include_bytes!("../ui/dist/marketplace.css"),
        ))
    }
    #[get("marketplace.sample-art", "/sample-assets/{name}")]
    async fn sample_art(
        &self,
        Path(path): Path<SampleAssetPath>,
    ) -> Result<HandleResponse, EndpointHandleInvocationError> {
        let bytes: &[u8] = match path.name.as_str() {
            "projects.png" => include_bytes!("../ui/assets/projects.png"),
            "observe.png" => include_bytes!("../ui/assets/observe.png"),
            "git.png" => include_bytes!("../ui/assets/git.png"),
            "files.png" => include_bytes!("../ui/assets/files.png"),
            "notes.png" => include_bytes!("../ui/assets/notes.png"),
            "echo.png" => include_bytes!("../ui/assets/echo.png"),
            "projects-preview.png" => include_bytes!("../ui/assets/projects-preview.png"),
            _ => {
                return Ok(response::problem(
                    StatusCode::NOT_FOUND,
                    "asset_not_found",
                    "Sample image does not exist",
                ));
            }
        };
        Ok(asset("image/png", bytes))
    }
    /// Public immutable signed metadata; consumers retain their own trust policy.
    #[get("marketplace.snapshot", "/api/marketplace/v1/snapshot")]
    async fn snapshot(
        &self,
        context: InvocationContext,
    ) -> Result<HandleResponse, EndpointHandleInvocationError> {
        let snapshot = self
            .directory
            .read_snapshot_with_context(context, directory::ReadSnapshotRequest {})
            .await
            .map_err(|error| {
                EndpointHandleInvocationError::Runtime(failure(format!(
                    "directory unavailable: {error:?}"
                )))
            })?;
        Ok(asset(
            "application/json",
            snapshot.envelope_json.as_str().as_bytes(),
        ))
    }
    #[get("marketplace.search", "/api/marketplace/v1/plugins")]
    async fn search(
        &self,
        context: InvocationContext,
        Query(search): Query<Search>,
    ) -> Result<HandleResponse, EndpointHandleInvocationError> {
        if search.q.len() > 256
            || !(1..=100).contains(&search.limit)
            || search.publisher.as_ref().is_some_and(|v| v.len() > 256)
            || search.license.as_ref().is_some_and(|v| v.len() > 256)
            || search
                .ids
                .as_ref()
                .is_some_and(|v| v.len() > 14000 || v.split(',').count() > 50)
        {
            return Ok(response::problem(
                StatusCode::BAD_REQUEST,
                "invalid_search",
                "Search exceeds supported bounds",
            ));
        }
        let (catalog, cached) = match self.catalog(context).await {
            Ok(catalog) => catalog,
            Err(_) => {
                return Ok(response::problem(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "catalog_unavailable",
                    "A current verified catalog is unavailable",
                ));
            }
        };
        let listed: Vec<_> = catalog
            .snapshot()
            .releases
            .iter()
            .filter(|release| {
                release.availability == lenso_marketplace_catalog::Availability::Listed
            })
            .collect();
        let publishers: std::collections::BTreeSet<_> =
            listed.iter().map(|r| r.publisher_id.clone()).collect();
        let licenses: std::collections::BTreeSet<_> =
            listed.iter().map(|r| r.license.clone()).collect();
        let query = search.q.to_lowercase();
        let ids: Option<std::collections::BTreeSet<_>> =
            search.ids.as_ref().map(|value| value.split(',').collect());
        let mut matches: Vec<_> = listed
            .into_iter()
            .filter(|r| {
                (query.is_empty()
                    || r.plugin_id.contains(&query)
                    || r.title.to_lowercase().contains(&query)
                    || r.summary.to_lowercase().contains(&query))
                    && search
                        .publisher
                        .as_ref()
                        .is_none_or(|publisher| publisher == &r.publisher_id)
                    && search
                        .license
                        .as_ref()
                        .is_none_or(|license| license == &r.license)
                    && ids.as_ref().is_none_or(|ids| {
                        ids.contains(format!("{}@{}", r.plugin_id, r.version).as_str())
                    })
            })
            .collect();
        matches.sort_by_key(|r| (r.title.to_lowercase(), &r.plugin_id, &r.version));
        let total = matches.len();
        let releases: Vec<_> = matches
            .into_iter()
            .skip(search.offset)
            .take(search.limit)
            .collect();
        Ok(response::json(
            StatusCode::OK,
            &serde_json::json!({"catalog_id":catalog.snapshot().catalog_id,"revision":catalog.snapshot().revision,"cached":cached,"total":total,"publishers":publishers,"licenses":licenses,"releases":releases}),
        )?)
    }
    #[get(
        "marketplace.release",
        "/api/marketplace/v1/plugins/{plugin_id}/{version}"
    )]
    async fn release(
        &self,
        context: InvocationContext,
        Path(path): Path<ReleasePath>,
    ) -> Result<HandleResponse, EndpointHandleInvocationError> {
        let (catalog, cached) = match self.catalog(context).await {
            Ok(catalog) => catalog,
            Err(_) => {
                return Ok(response::problem(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "catalog_unavailable",
                    "A current verified catalog is unavailable",
                ));
            }
        };
        match catalog
            .snapshot()
            .releases
            .iter()
            .find(|release| release.plugin_id == path.plugin_id && release.version == path.version)
        {
            Some(release) => Ok(response::json(
                StatusCode::OK,
                &serde_json::json!({"catalog_id":catalog.snapshot().catalog_id,"cached":cached,"release":release}),
            )?),
            None => Ok(response::problem(
                StatusCode::NOT_FOUND,
                "release_not_found",
                "Exact release is not in this catalog",
            )),
        }
    }
}
fn failure(detail: impl std::fmt::Display) -> RuntimeFailure {
    RuntimeFailure::PluginFailure {
        detail: detail.to_string(),
    }
}
pub fn link() {}

fn asset(content_type: &str, bytes: &[u8]) -> HandleResponse {
    use lenso_capability_http_endpoint::HandleResponseHeadersItem;
    // Exact Base UI 1.7.0 scrollbar stylesheet emitted by Select. Keep inline
    // script and arbitrary style execution blocked; browser tests gate upgrades.
    const POLICY: &str = "default-src 'self'; img-src 'self' https:; script-src 'self'; style-src 'self'; style-src-elem 'self' 'sha256-kLmvWqfziFavKtqHqRsb90f006UAK2Dmd0It5Iz2KFA='; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";
    let headers = [
        ("content-type", content_type),
        ("cache-control", "no-cache"),
        ("content-security-policy", POLICY),
    ]
    .into_iter()
    .map(|(name, value)| HandleResponseHeadersItem {
        name: name.into(),
        value: value.into(),
    })
    .collect();
    HandleResponse {
        status: 200,
        body: bytes.to_vec().into(),
        headers,
    }
}
