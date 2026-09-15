//! Reference marketplace Host assembly. No Console Shell dependency.
use lenso_app_plan::authoring::{
    HostBinding, HostCatalog, HostDefaultPlugin, HostPluginRelease, HostSlot, PluginInstanceId,
};
use lenso_kernel::{Kernel, NativeApp};
use lenso_native_adapter::NativePluginRegistry;
use lenso_web_ingress_plugin::{WebIngressConfig, WebIngressFactory};
use std::{io::Write as _, net::SocketAddr, path::PathBuf};

#[derive(Clone, Debug)]
pub struct Config {
    pub app_root: PathBuf,
    pub directory_database: PathBuf,
    pub catalog_id: String,
    pub key_id: String,
    pub public_key_hex: String,
    pub address: SocketAddr,
}

pub async fn start(config: &Config) -> anyhow::Result<(NativeApp, SocketAddr)> {
    anyhow::ensure!(
        config.app_root.is_absolute() && config.app_root.parent().is_some(),
        "App root must be an absolute non-root path"
    );
    lenso_marketplace_directory_plugin::link();
    lenso_marketplace_web_plugin::link();
    let ingress = WebIngressFactory::new().with_diagnostics(Diagnostics);
    let slots = [
        HostSlot::many("http-ingress"),
        HostSlot::one("marketplace-directories"),
        HostSlot::many("web"),
    ];
    let linked = NativePluginRegistry::host_catalog(slots.clone(), [])
        .map_err(|e| anyhow::anyhow!("linked catalog: {e:?}"))?;
    let mut releases = linked.plugins().to_vec();
    releases.push(HostPluginRelease::new(
        WebIngressFactory::plugin_descriptor(),
    ));
    let ingress_config = WebIngressConfig::default()
        .with_bind_address(config.address)
        .map_err(anyhow::Error::msg)?;
    let defaults=[
        HostDefaultPlugin::new("lenso.web-ingress","default").with_configuration(serde_json::to_value(ingress_config)?),
        HostDefaultPlugin::new("lenso.marketplace.directory","default").with_configuration(serde_json::json!({"database":config.directory_database,"catalog_id":config.catalog_id})),
        HostDefaultPlugin::new("lenso.marketplace.web","default").with_configuration(serde_json::json!({"catalog_id":config.catalog_id,"key_id":config.key_id,"public_key_hex":config.public_key_hex,"cache_database":config.app_root.join("verified-catalog.sqlite3")})).disableable(),
    ];
    let host = HostCatalog::new(slots, releases, defaults).with_bindings([
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
    let control = config.app_root.join(".lenso");
    std::fs::create_dir_all(&control)?;
    std::fs::create_dir_all(config.app_root.join("plugins"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(&control)?;
    temporary.write_all(&serde_json::to_vec_pretty(&host)?)?;
    temporary.as_file().sync_all()?;
    temporary.persist(control.join("host-catalog.json"))?;
    let resolved = lenso_app_authoring::load_resolved_app(&config.app_root)?;
    let registry = NativePluginRegistry::new()
        .with_linked_factories()
        .with_factory(ingress.clone());
    let app = Kernel::start_native(
        resolved.plan().clone(),
        lenso_runner::TokioDriver::new(),
        registry,
    )
    .await
    .map_err(|e| anyhow::anyhow!("marketplace startup: {e:?}"))?;
    let address = ingress
        .local_address()
        .ok_or_else(|| anyhow::anyhow!("marketplace ingress has no address"))?;
    Ok((app, address))
}

#[derive(Debug)]
struct Diagnostics;
impl lenso_web_ingress_plugin::WebIngressDiagnostics for Diagnostics {
    fn endpoint_runtime_failure(
        &self,
        event: lenso_web_ingress_plugin::WebIngressEndpointFailure<'_>,
    ) {
        eprintln!(
            "marketplace endpoint {}: {:?}",
            event.route_id(),
            event.failure()
        );
    }
}
