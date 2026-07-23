//! build.rs — Item registry integrity check
//!
//! Validates that the generated `item_registry.in.rs` contains the expected
//! number of entries and passes basic consistency checks at compile time.
//!
//! To regenerate the registry:
//!   ./scripts/update-minecraft-data.sh 26.2 --apply

use std::fs;
use std::path::Path;

fn main() {
    let registry_path = Path::new("src/item_registry.in.rs");

    // Only validate if the file exists (it's checked in, so it always should)
    if !registry_path.exists() {
        println!("cargo:warning=item_registry.in.rs not found — registry will be empty!");
        return;
    }

    let content = fs::read_to_string(registry_path)
        .expect("Failed to read item_registry.in.rs");

    // Count insert entries
    let insert_count = content.matches("m.insert(").count();
    let block_comments = content.matches("// ═══ BLOCKS").count();
    let item_comments = content.matches("// ═══ ITEMS").count();

    println!("cargo:rustc-env=MC_ITEM_REGISTRY_SIZE={}", insert_count);

    // Basic validation
    if insert_count < 1500 {
        panic!(
            "Item registry too small: {} entries. Expected >= 1500. \
             Run: ./scripts/update-minecraft-data.sh 26.2 --apply",
            insert_count
        );
    }
    if insert_count > 2000 {
        panic!(
            "Item registry suspiciously large: {} entries. Expected <= 2000.",
            insert_count
        );
    }
    if block_comments == 0 || item_comments == 0 {
        println!(
            "cargo:warning=Registry structure unexpected: blocks={}, items={}",
            block_comments, item_comments
        );
    }

    // Verify the file has the closing brace (function ends with `}`)
    if !content.trim_end().ends_with("}") {
        panic!("item_registry.in.rs must end with `}}` (closing function brace)");
    }

    println!(
        "cargo:warning=Item registry: {} entries validated",
        insert_count
    );
}
