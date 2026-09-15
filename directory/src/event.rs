//! Event-owned binding injection. Endpoint and lifecycle projections are generated
//! from the same Plugin source as the native factory.
use super::*;
use lenso_native_adapter::{
    ConfiguredPluginFactory, NativePluginFactory, NativePluginFactoryContext, NativePluginInstance,
};

#[derive(Clone, Debug)]
pub struct EventDirectoryFactory {
    binding: String,
    storage: Rc<dyn SnapshotStorage>,
}
impl EventDirectoryFactory {
    pub fn new(binding: String, storage: Rc<dyn SnapshotStorage>) -> Self {
        Self { binding, storage }
    }
}
impl NativePluginFactory for EventDirectoryFactory {
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
        let owner = self.clone();
        ConfiguredPluginFactory::<MarketplaceDirectory, _>::new(move |plugin| {
            if plugin.config.storage_binding.as_deref() != Some(owner.binding.as_str()) {
                return Err(failure("directory storage binding mismatch"));
            }
            plugin.storage = Some(owner.storage.clone());
            Ok(())
        })
        .instantiate(context)
    }
}
