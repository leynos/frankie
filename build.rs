//! Build script that ensures Cargo rebuilds when migrations change.
//!
//! The `embed_migrations!` macro reads migration files at compile time but
//! Cargo cannot automatically detect when those files change. This script
//! emits `rerun-if-changed` directives so incremental builds pick up new
//! or modified migrations.

fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
