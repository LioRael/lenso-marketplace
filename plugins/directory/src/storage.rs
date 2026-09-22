//! Directory-owned host storage port. No Web cache or installation state.
use futures::future::LocalBoxFuture;
pub trait PublishedStorage: std::fmt::Debug {
    fn published<'a>(
        &'a self,
        catalog: &'a str,
    ) -> LocalBoxFuture<'a, anyhow::Result<Option<String>>>;

    fn published_details<'a>(
        &'a self,
        _catalog: &'a str,
    ) -> LocalBoxFuture<'a, anyhow::Result<Option<String>>> {
        Box::pin(async { Ok(None) })
    }
}
