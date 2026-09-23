use anyhow::{Result, bail, ensure};
use std::{io, path::Path};
fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: lenso-marketplace-author prepare ARCHIVE METADATA.json NEW_DIRECTORY | check SUBMISSION_DIRECTORY | submission-url SUBMISSION_DIRECTORY | prepare-linked-cargo PACKAGE.crate METADATA.json NEW_DIRECTORY | check-linked-cargo SUBMISSION_DIRECTORY";
    if args.as_slice() == ["--help"] {
        println!("{usage}");
        return Ok(());
    }
    if args.as_slice() == ["--version"] {
        println!("lenso-marketplace-author {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args
        .first()
        .is_some_and(|command| command == "prepare-linked-cargo")
    {
        ensure!(args.len() == 4, usage);
        let release = lenso_marketplace_publisher::prepare_linked_cargo(
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
        )?;
        serde_json::to_writer(
            io::stdout().lock(),
            &serde_json::json!({
                "status":"prepared", "plugin_id":release.plugin_id,
                "version":release.version, "crate_digest":release.crate_digest,
                "note":"Local validation only. Submit the exact .crate and release.json for namespace review and publication."
            }),
        )?;
        return Ok(());
    }
    if args
        .first()
        .is_some_and(|command| command == "check-linked-cargo")
    {
        ensure!(args.len() == 2, usage);
        let release = lenso_marketplace_publisher::check_linked_cargo(Path::new(&args[1]))?;
        serde_json::to_writer(
            io::stdout().lock(),
            &serde_json::json!({
                "status":"verified", "plugin_id":release.plugin_id,
                "version":release.version, "crate_digest":release.crate_digest,
                "note":"Local validation only. Registry provenance and Host build behavior require independent review."
            }),
        )?;
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
        Some("check" | "submission-url") => {
            ensure!(args.len() == 2, usage);
            lenso_marketplace_publisher::check(Path::new(&args[1]))?.0
        }
        _ => bail!(usage),
    };
    if args[0] == "submission-url" {
        let mut url = url::Url::parse("https://github.com/LioRael/lenso-marketplace/issues/new")?;
        url.query_pairs_mut().extend_pairs([
            ("template", "plugin-submission.yml"),
            (
                "title",
                &format!("[Plugin] {} {}", release.plugin_id, release.version),
            ),
            ("plugin_id", &release.plugin_id),
            ("version", &release.version),
            ("publisher", &release.publisher_id),
            ("source", &release.source_url),
            ("revision", &release.source_revision),
            ("artifact", &release.artifact.url),
            ("digest", &release.artifact.digest),
        ]);
        println!("{url}");
        return Ok(());
    }
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
