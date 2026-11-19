//! Rebuilds the crate when a migration is added: the macro that embeds them only watches
//! the files that already existed. It also reruns when the image gives the hub the number and
//! the digests of the connectors it was built with (`bundle`).

fn main() {
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-env-changed=ARAUTO_BUNDLE_CONNECTOR_VERSION");
    println!("cargo:rerun-if-env-changed=ARAUTO_BUNDLE_CONNECTOR_SHA256");
}
