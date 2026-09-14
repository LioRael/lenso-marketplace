//! Marketplace directory native Plugin (public read role).

use lenso_capability_marketplace_directory as contract;
use lenso_kernel::{
    DeactivateContext, InvocationContext, NativeRequestFuture, PrepareContext, RuntimeFailure,
};
use lenso_marketplace_catalog::directory::PublishedDirectory;
use std::{cell::RefCell, path::PathBuf, rc::Rc};

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    database: PathBuf,
    catalog_id: String,
}

#[lenso::plugin(lifecycle, configuration_schema = "config.schema.json")]
#[derive(Clone, Debug)]
struct MarketplaceDirectory {
    #[config]
    config: Config,
    reader: Rc<RefCell<Option<PublishedDirectory>>>,
    #[tasks]
    tasks: lenso::ManagedTasks,
}

impl MarketplaceDirectory {
    fn open(&self) -> Result<(), RuntimeFailure> {
        if self.reader.borrow().is_some() {
            return Err(failure("directory prepared twice"));
        }
        let reader = PublishedDirectory::open(&self.config.database, &self.config.catalog_id)
            .map_err(failure)?;
        self.reader.replace(Some(reader));
        Ok(())
    }
    fn close(&self) {
        self.reader.take();
    }
}

impl lenso::Lifecycle for MarketplaceDirectory {
    async fn prepare(&self, _context: PrepareContext) -> Result<(), RuntimeFailure> {
        self.open()
    }
    async fn deactivate(&self, _context: DeactivateContext) -> Result<(), RuntimeFailure> {
        self.close();
        Ok(())
    }
}

#[lenso::provides(contract::Directory)]
impl MarketplaceDirectory {
    fn read_snapshot(
        &self,
        _context: InvocationContext,
        _request: contract::ReadSnapshotRequest,
    ) -> NativeRequestFuture<contract::Directory> {
        let result = match self.reader.borrow().as_ref() {
            None => Err(RuntimeFailure::Unavailable {
                capability: contract::CAPABILITY_ID,
            }),
            Some(reader) => reader
                .latest()
                .map_err(failure)
                .and_then(|snapshot| match snapshot {
                    Some(envelope_json) => Ok(Ok(contract::ReadSnapshotResponse {
                        envelope_json: envelope_json.try_into().map_err(failure)?,
                    })),
                    None => Ok(Err(contract::ReadSnapshotError::NotPublished)),
                }),
        };
        Box::pin(async move { result })
    }
}

fn failure(detail: impl std::fmt::Display) -> RuntimeFailure {
    RuntimeFailure::PluginFailure {
        detail: detail.to_string(),
    }
}

/// Makes the generated native factory available; the Host chooses activation.
pub fn link() {}

#[cfg(test)]
mod tests {
    use super::*;
    use lenso_kernel::{CancellationToken, NativeRequestEndpoint};
    use lenso_marketplace_catalog::{Trust, directory::Directory, verify};
    use std::collections::{BTreeMap, BTreeSet};

    fn plugin(database: PathBuf) -> MarketplaceDirectory {
        MarketplaceDirectory {
            config: Config {
                database,
                catalog_id: "test".into(),
            },
            reader: Rc::default(),
            tasks: lenso::ManagedTasks::default(),
        }
    }

    #[test]
    fn generated_endpoint_reads_published_bytes_and_stops_after_close() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("marketplace.db");
        let mut publisher =
            Directory::open(&database, "test", BTreeSet::from(["reviewer".into()])).unwrap();
        let provider = plugin(database);
        provider.open().unwrap();
        let endpoint = contract::DirectoryEndpoint::new(provider.clone());
        let invoke = || {
            futures::executor::block_on(endpoint.invoke(
                contract::READ_SNAPSHOT_OPERATION,
                Box::new(contract::ReadSnapshotRequest {}),
                InvocationContext::new(1, None, CancellationToken::new()),
            ))
        };
        let error = invoke()
            .unwrap()
            .unwrap_err()
            .downcast::<contract::ReadSnapshotError>()
            .unwrap();
        assert_eq!(*error, contract::ReadSnapshotError::NotPublished);
        let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
        let published = publisher
            .publish("reviewer", 0, 100, 200, "test-key", &key)
            .unwrap();
        let response = invoke()
            .unwrap()
            .unwrap()
            .downcast::<contract::ReadSnapshotResponse>()
            .unwrap();
        assert_eq!(response.envelope_json.as_str().as_bytes(), published);
        verify(
            response.envelope_json.as_str().as_bytes(),
            &Trust {
                catalog_id: "test".into(),
                keys: BTreeMap::from([("test-key".into(), key.verifying_key())]),
            },
            None,
            150,
        )
        .unwrap();
        provider.close();
        assert!(matches!(invoke(), Err(RuntimeFailure::Unavailable { .. })));
        provider.open().unwrap();
        assert!(invoke().unwrap().is_ok());
    }

    #[test]
    fn missing_database_fails_without_creating_an_empty_marketplace() {
        let root = tempfile::tempdir().unwrap();
        let database = root.path().join("missing.db");
        assert!(plugin(database.clone()).open().is_err());
        assert!(!database.exists());
    }
}
