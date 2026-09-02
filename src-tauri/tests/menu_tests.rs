use std::fs;
use std::path::Path;

#[test]
fn test_all_declared_menu_items_have_shell_handlers() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let menu_rs_path = Path::new(manifest_dir).join("src/menu.rs");
    let shell_js_path = Path::new(manifest_dir).join("../host/shell.js");

    let menu_src = fs::read_to_string(&menu_rs_path)
        .expect("Failed to read src/menu.rs");
    let shell_src = fs::read_to_string(&shell_js_path)
        .expect("Failed to read host/shell.js");

    // Extract all menu IDs declared via MenuItemBuilder::with_id("...", ...)
    let mut menu_ids = Vec::new();
    for line in menu_src.lines() {
        if let Some(pos) = line.find("MenuItemBuilder::with_id(") {
            let rest = &line[pos + "MenuItemBuilder::with_id(".len()..];
            if let Some(quote_start) = rest.find('"') {
                let after_quote = &rest[quote_start + 1..];
                if let Some(quote_end) = after_quote.find('"') {
                    let id = &after_quote[..quote_end];
                    menu_ids.push(id.to_string());
                }
            }
        }
    }

    assert!(
        menu_ids.len() >= 25,
        "Expected at least 25 declared native menu items, found {}",
        menu_ids.len()
    );

    println!("Verifying {} native menu items against host/shell.js handlers...", menu_ids.len());

    let mut missing_handlers = Vec::new();
    for id in &menu_ids {
        let pattern1 = format!("\"{}\"", id);
        let pattern2 = format!("'{}'", id);
        if !shell_src.contains(&pattern1) && !shell_src.contains(&pattern2) {
            missing_handlers.push(id.clone());
        }
    }

    assert!(
        missing_handlers.is_empty(),
        "The following menu item IDs in menu.rs are missing handlers in host/shell.js: {:?}",
        missing_handlers
    );

    println!("✓ All {} native menu items have corresponding handlers in host/shell.js!", menu_ids.len());
}

#[test]
fn test_menu_structure_contains_all_core_categories() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let menu_rs_path = Path::new(manifest_dir).join("src/menu.rs");
    let menu_src = fs::read_to_string(&menu_rs_path)
        .expect("Failed to read src/menu.rs");

    let required_submenus = ["File", "Security & Auth", "Apps", "Storage & Data", "View", "Help"];
    for submenu in required_submenus {
        assert!(
            menu_src.contains(&format!("SubmenuBuilder::new(app, \"{}\")", submenu))
                || menu_src.contains(&format!("\"{}\"", submenu)),
            "Menu definition in src/menu.rs must include '{}' submenu",
            submenu
        );
    }
}
