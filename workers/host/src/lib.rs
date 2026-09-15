use futures::future::LocalBoxFuture;
use lenso_app_plan::authoring::{
    HostBinding, HostCatalog, HostDefaultPlugin, HostPluginRelease, HostSlot, PluginInstanceId,
    PluginRootSnapshot, resolve_plugin_root,
};
use lenso_kernel::{CancellationToken, Kernel, RuntimeFailure, ShutdownOutcome};
use lenso_marketplace_catalog::persistence::{AcceptedEnvelope, SnapshotStorage};
use lenso_marketplace_directory_plugin::EventDirectoryFactory;
use lenso_marketplace_web_plugin::{EventWebFactory, MarketplaceClock};
use lenso_native_adapter::NativePluginRegistry;
use lenso_web_ingress_plugin::{WebIngressConfig, WebIngressEventFactory};
use lenso_workers_driver::WorkersDriver;
use std::{rc::Rc, time::Duration};
use wasm_bindgen::{JsCast, prelude::*};

fn error(value: impl std::fmt::Debug) -> JsValue {
    JsValue::from_str(&format!("{value:?}"))
}
fn failure() -> RuntimeFailure {
    RuntimeFailure::PluginFailure {
        detail: "Marketplace event host failed".into(),
    }
}
#[derive(Debug)]
struct Storage(js_sys::Function);
impl Storage {
    async fn invoke<T: serde::de::DeserializeOwned>(
        &self,
        operation: &str,
        input: serde_json::Value,
    ) -> anyhow::Result<T> {
        let promise = self
            .0
            .call2(
                &JsValue::NULL,
                &JsValue::from_str(operation),
                &JsValue::from_str(&input.to_string()),
            )
            .map_err(|_| anyhow::anyhow!("storage invocation failed"))?;
        let value = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&promise))
            .await
            .map_err(|_| anyhow::anyhow!("storage operation failed"))?;
        serde_json::from_str(
            &value
                .as_string()
                .ok_or_else(|| anyhow::anyhow!("invalid storage response"))?,
        )
        .map_err(Into::into)
    }
}
impl SnapshotStorage for Storage {
    fn published<'a>(
        &'a self,
        catalog: &'a str,
    ) -> LocalBoxFuture<'a, anyhow::Result<Option<String>>> {
        Box::pin(self.invoke("published", serde_json::json!({"catalog":catalog})))
    }
    fn accepted<'a>(
        &'a self,
        catalog: &'a str,
    ) -> LocalBoxFuture<'a, anyhow::Result<Option<AcceptedEnvelope>>> {
        Box::pin(self.invoke("accepted", serde_json::json!({"catalog":catalog})))
    }
    fn compare_exchange<'a>(
        &'a self,
        catalog: &'a str,
        expected: Option<&'a str>,
        value: &'a AcceptedEnvelope,
    ) -> LocalBoxFuture<'a, anyhow::Result<bool>> {
        Box::pin(self.invoke(
            "compare_exchange",
            serde_json::json!({"catalog":catalog,"expected":expected,"value":value}),
        ))
    }
}
#[derive(Debug)]
struct Clock;
impl MarketplaceClock for Clock {
    fn now(&self) -> Result<u64, RuntimeFailure> {
        let now = js_sys::Date::now();
        if !now.is_finite() || now < 0.0 {
            return Err(failure());
        }
        Ok((now / 1000.0).floor() as u64)
    }
}
struct Guard(WorkersDriver);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.request_shutdown();
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    method: String,
    uri: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    catalog_id: String,
    key_id: String,
    public_key_hex: String,
    #[serde(default)]
    diagnostics: bool,
}

#[wasm_bindgen(raw_module = "@lenso/workers-runtime/http")]
extern "C" {
    #[wasm_bindgen(js_name = cancellation)]
    fn attach_cancellation(scope: &JsValue, callback: &JsValue);

}

#[wasm_bindgen]
pub async fn handle_http(input: String, scope: JsValue) -> Result<String, JsValue> {
    let request: Request = serde_json::from_str(&input).map_err(error)?;
    if request.body.len() > 65536 || request.headers.len() > 128 {
        return Err(error("request bound"));
    }
    let configuration = js_sys::Reflect::get(&scope, &"configuration".into()).map_err(error)?;
    let config: Config = serde_json::from_str(
        &configuration
            .as_string()
            .ok_or_else(|| error("configuration missing"))?,
    )
    .map_err(error)?;
    let storage = Rc::new(Storage(
        js_sys::Reflect::get(&scope, &"storage".into())
            .map_err(error)?
            .dyn_into()
            .map_err(error)?,
    ));
    let ingress = WebIngressEventFactory::new();
    let releases = vec![
        HostPluginRelease::new(WebIngressEventFactory::plugin_descriptor()),
        HostPluginRelease::new(
            serde_json::from_str(lenso_marketplace_directory_plugin::PLUGIN_DESCRIPTOR_JSON)
                .map_err(error)?,
        ),
        HostPluginRelease::new(
            serde_json::from_str(lenso_marketplace_web_plugin::PLUGIN_DESCRIPTOR_JSON)
                .map_err(error)?,
        ),
    ];
    let defaults = [
        HostDefaultPlugin::new("lenso.web-ingress", "default").with_configuration(serde_json::to_value(WebIngressConfig::default()).map_err(error)?),
        HostDefaultPlugin::new("lenso.marketplace.directory", "default").with_configuration(serde_json::json!({"catalog_id":config.catalog_id,"storage_binding":"MARKETPLACE"})),
        HostDefaultPlugin::new("lenso.marketplace.web", "default").with_configuration(serde_json::json!({"catalog_id":config.catalog_id,"key_id":config.key_id,"public_key_hex":config.public_key_hex,"storage_binding":"MARKETPLACE"})),
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
    let plan = resolve_plugin_root(&host, &PluginRootSnapshot::default()).map_err(error)?;
    lenso_marketplace_directory_plugin::link_plugin();
    lenso_marketplace_web_plugin::link_plugin();
    let registry = NativePluginRegistry::new()
        .with_factory(ingress.clone())
        .with_factory_override(EventDirectoryFactory::new(
            "MARKETPLACE".into(),
            storage.clone(),
        ))
        .map_err(error)?
        .with_factory_override(
            EventWebFactory::new("MARKETPLACE".into(), storage, Rc::new(Clock))
                .with_diagnostics(Rc::new(Diagnostics(config.diagnostics))),
        )
        .map_err(error)?
        .with_linked_factories();
    let driver = WorkersDriver::new();
    let _guard = Guard(driver.clone());
    let app = Kernel::start_native(plan.plan().clone(), driver, registry)
        .await
        .map_err(error)?;
    let cancellation = CancellationToken::new();
    let token = cancellation.clone();
    let on_cancel = Closure::<dyn FnMut()>::new(move || {
        token.cancel();
    });
    attach_cancellation(&scope, on_cancel.as_ref());
    let outcome = async {
        let mut builder = http::Request::builder()
            .method(request.method.as_str())
            .uri(request.uri);
        for (name, value) in request.headers {
            builder = builder.header(name, value);
        }
        let request = builder
            .body(bytes::Bytes::from(request.body))
            .map_err(error)?;
        ingress.handle(request, cancellation).await.map_err(error)
    }
    .await;
    attach_cancellation(&scope, &JsValue::NULL);
    drop(on_cancel);
    let shutdown = app.shutdown(Duration::from_secs(1)).await;
    if !matches!(shutdown, ShutdownOutcome::Clean) {
        return Err(error("unclean Marketplace shutdown"));
    }
    let response = outcome?;
    if response.body().len() > 4 * 1024 * 1024 {
        return Err(error("response exceeds bound"));
    }
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| Ok((name.as_str(), value.to_str().map_err(error)?)))
        .collect::<Result<Vec<_>, JsValue>>()?;
    // A JSON array allocates one Value per byte and can exceed the isolate's
    // memory budget for an otherwise bounded response. Encode the boundary once.
    let body = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, response.body());
    Ok(serde_json::json!({"status":response.status().as_u16(),"headers":headers,"body_base64":body,"ready":true,"shutdown":"clean"}).to_string())
}

#[derive(Debug)]
struct Diagnostics(bool);
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    fn report_catalog_failure(message: &str);
}
impl lenso_marketplace_web_plugin::MarketplaceDiagnostics for Diagnostics {
    fn catalog_failure(&self, error: &RuntimeFailure) {
        if self.0 {
            report_catalog_failure(&format!("Marketplace proof verification: {error:?}"));
        }
    }
}
