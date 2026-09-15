use lenso_contract_codegen::{ProjectionLanguage, check_projection, write_projection};
use std::{env, path::Path};
fn main() {
    for path in [
        "capability.json",
        "schemas",
        "src/generated.rs",
        "generated/bindings.ts",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=LENSO_UPDATE_CONTRACT_SNAPSHOT");
    for (language, output) in [
        (ProjectionLanguage::Rust, "src/generated.rs"),
        (ProjectionLanguage::TypeScript, "generated/bindings.ts"),
    ] {
        if env::var_os("LENSO_UPDATE_CONTRACT_SNAPSHOT").is_some() {
            write_projection(Path::new("capability.json"), language, Path::new(output))
                .expect("generate directory contract");
        } else {
            check_projection(Path::new("capability.json"), language, Path::new(output))
                .expect("directory contract projection is stale");
        }
    }
}
