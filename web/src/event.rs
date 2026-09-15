//! Explicit event-owned storage and wall clock for the shared Web Plugin.
use super::*;
use lenso_native_adapter::{NativePluginFactory, NativePluginFactoryContext, NativePluginInstance};

pub trait MarketplaceClock: std::fmt::Debug {
    fn now(&self) -> Result<u64, RuntimeFailure>;
}

/// Host-owned private diagnostics; public HTTP responses remain sanitized.
pub trait MarketplaceDiagnostics: std::fmt::Debug {
    fn catalog_failure(&self, error: &RuntimeFailure);
}

#[derive(Clone, Debug)]
pub struct EventWebFactory {
    binding: String,
    storage: Rc<dyn SnapshotStorage>,
    clock: Rc<dyn MarketplaceClock>,
    diagnostics: Option<Rc<dyn MarketplaceDiagnostics>>,
}
impl EventWebFactory {
    pub fn new(
        binding: String,
        storage: Rc<dyn SnapshotStorage>,
        clock: Rc<dyn MarketplaceClock>,
    ) -> Self {
        Self {
            binding,
            storage,
            clock,
            diagnostics: None,
        }
    }
    pub fn with_diagnostics(mut self, diagnostics: Rc<dyn MarketplaceDiagnostics>) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }
}
impl NativePluginFactory for EventWebFactory {
    fn package_id(&self) -> &'static str {
        PACKAGE_ID
    }
    fn package_version(&self) -> &'static str {
        PACKAGE_VERSION
    }
    fn instantiate(
        &self,
        context: NativePluginFactoryContext<'_>,
    ) -> Result<NativePluginInstance, RuntimeFailure> {
        let mut plugin = MarketplaceWeb::__lenso_construct(context)?;
        if plugin.config.storage_binding.as_deref() != Some(self.binding.as_str()) {
            return Err(failure("Web storage binding mismatch"));
        }
        plugin.storage = Some(self.storage.clone());
        plugin.clock = Some(self.clock.clone());
        plugin.diagnostics = self.diagnostics.clone();
        let endpoint = http_endpoint_contract::EndpointEndpoint::new(plugin.clone());
        Ok(NativePluginInstance::with_lifecycle(
            vec![Rc::new(endpoint)],
            __LensoLifecycleMarketplaceWeb {
                plugin: Rc::new(plugin),
            },
        ))
    }
}
