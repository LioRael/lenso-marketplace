use clap::Parser;
#[derive(Parser)]
#[command(about = "Run the Lenso Plugin marketplace public read service")]
struct Arguments {
    #[arg(long)]
    app_root: std::path::PathBuf,
    #[arg(long)]
    directory_database: std::path::PathBuf,
    #[arg(long)]
    catalog_id: String,
    #[arg(long)]
    key_id: String,
    #[arg(long)]
    public_key_hex: String,
    #[arg(long, default_value = "127.0.0.1:0")]
    listen: std::net::SocketAddr,
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Arguments::parse();
    tokio::task::LocalSet::new()
        .run_until(async move {
            let config = lenso_marketplace_app::Config {
                app_root: args.app_root,
                directory_database: args.directory_database,
                catalog_id: args.catalog_id,
                key_id: args.key_id,
                public_key_hex: args.public_key_hex,
                address: args.listen,
            };
            let (app, address) = lenso_marketplace_app::start(&config).await?;
            println!("Lenso marketplace listening on http://{address}");
            tokio::signal::ctrl_c().await?;
            match app.shutdown(std::time::Duration::from_secs(10)).await {
                lenso_kernel::ShutdownOutcome::Clean => Ok(()),
                other => anyhow::bail!("marketplace shutdown: {other:?}"),
            }
        })
        .await
}
