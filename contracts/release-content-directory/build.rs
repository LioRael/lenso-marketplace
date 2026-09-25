use lenso_contract_codegen::{ProjectionLanguage, check_projection};
use std::path::Path;

fn main() {
    for path in [
        "capability.json",
        "schemas",
        "src/generated.rs",
        "generated/bindings.ts",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    for (language, output) in [
        (ProjectionLanguage::Rust, "src/generated.rs"),
        (ProjectionLanguage::TypeScript, "generated/bindings.ts"),
    ] {
        check_projection(Path::new("capability.json"), language, Path::new(output))
            .expect("release content directory contract projection is stale");
    }
}
