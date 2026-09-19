use anyhow::{Result, bail, ensure};
use std::{io, path::Path};
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: lenso-marketplace-author prepare ARCHIVE METADATA.json NEW_DIRECTORY | check SUBMISSION_DIRECTORY";
    if args.as_slice() == ["--help"] {
        println!("{usage}");
        return Ok(());
    }
    let release = match args.first().map(String::as_str) {
        Some("prepare") => {
            ensure!(args.len() == 4, usage);
            lenso_marketplace_publisher::prepare(
                Path::new(&args[1]),
                Path::new(&args[2]),
                Path::new(&args[3]),
            )?
        }
        Some("check") => {
            ensure!(args.len() == 2, usage);
            lenso_marketplace_publisher::check(Path::new(&args[1]))?.0
        }
        _ => bail!(usage),
    };
    serde_json::to_writer(
        io::stdout().lock(),
        &serde_json::json!({
            "status":if args[0] == "check" { "verified" } else { "prepared" }, "plugin_id":release.plugin_id, "version":release.version,
            "artifact_digest":release.artifact.digest,
            "note":"Local validation only. Submit both files to the catalog maintainer for namespace review and publication."
        }),
    )?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
