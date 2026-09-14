use sha2::{Digest, Sha256};
fn main() {
    let mut hash = Sha256::new();
    for file in [
        "main.tsx",
        "app.tsx",
        "components.tsx",
        "model.ts",
        "navigation.ts",
        "catalog.ts",
        "saved.ts",
        "sample.ts",
        "controls.ts",
        "catalog-layout.stylex.ts",
        "marketplace.css",
        "index.html",
        "vite.config.ts",
        "tsconfig.json",
        "../../../../pnpm-lock.yaml",
    ] {
        let path = format!("ui/{file}");
        println!("cargo:rerun-if-changed={path}");
        hash.update(file.as_bytes());
        hash.update(b"\0");
        hash.update(std::fs::read(path).expect("marketplace UI source missing"));
        hash.update(b"\0");
    }
    println!("cargo:rerun-if-changed=ui/dist");
    let actual = format!("{:x}", hash.finalize());
    let built = std::fs::read_to_string("ui/dist/source.sha256").unwrap_or_default();
    assert_eq!(
        actual, built,
        "Marketplace UI is missing or stale. Run pnpm marketplace:build from the workspace root."
    );
}
