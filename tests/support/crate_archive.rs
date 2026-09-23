pub fn create(package: &str, version: &str, plugin_id: &str) -> Vec<u8> {
    let manifest = format!(
        "[package]\nname = \"{package}\"\nversion = \"{version}\"\n[package.metadata.lenso]\nplugin-id = \"{plugin_id}\"\n"
    );
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(
            &mut header,
            format!("{package}-{version}/Cargo.toml"),
            manifest.as_bytes(),
        )
        .unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}
