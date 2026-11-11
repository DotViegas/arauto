//! Rebuilds the crate when a migration is added: the macro that embeds them only watches
//! the files that already existed.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
