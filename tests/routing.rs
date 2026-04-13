#[test]
fn test_message_routing_exhaustiveness() {
    // Read the app module source files (split from the former monolithic app.rs)
    let mod_src = include_str!("../src/app/mod.rs");

    // Handler files — each contains a single update_* method
    let handler_sources: &[&str] = &[
        include_str!("../src/app/update.rs"),
        include_str!("../src/app/entries.rs"),
        include_str!("../src/app/work_day.rs"),
        include_str!("../src/app/settings.rs"),
        include_str!("../src/app/vacation.rs"),
        include_str!("../src/app/billable.rs"),
        include_str!("../src/app/stats.rs"),
    ];

    // 1. Extract the `Message` enum block to find all variants
    let enum_start = mod_src.find("pub enum Message {").expect("Could not find Message enum");
    let enum_block = &mod_src[enum_start..];
    let enum_end = enum_block.find("\n}\n").expect("Could not find end of Message enum");
    let enum_body = &enum_block[..enum_end];

    let mut variants = std::collections::HashSet::new();
    for line in enum_body.lines() {
        let trimmed = line.trim();
        // Skip comments, decorators, and empty lines
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with("pub enum") || trimmed.starts_with('#') {
            continue;
        }

        // Isolate the variant name (handle variants with data like `SettingsTokenChanged(String)`)
        if let Some(name) = trimmed.split(['(', '{', ',']).next() {
            let name = name.trim();
            if !name.is_empty() {
                variants.insert(name.to_string());
            }
        }
    }

    // 2. Scan all handler files for `Message::Variant` references.
    let mut handled_variants = std::collections::HashSet::new();

    for source in handler_sources {
        let mut search_idx = 0;
        while let Some(idx) = source[search_idx..].find("Message::") {
            let actual_idx = search_idx + idx;
            let rest = &source[actual_idx + 9..];

            if let Some(end_idx) = rest.find(|c: char| !c.is_alphanumeric() && c != '_') {
                let variant_name = &rest[..end_idx];
                if !variant_name.is_empty() {
                    handled_variants.insert(variant_name.to_string());
                }
            }
            search_idx = actual_idx + 9;
        }
    }

    // 3. Verify completeness
    let mut unhandled: Vec<_> = variants.iter().filter(|v| !handled_variants.contains(*v)).collect();
    unhandled.sort(); // For deterministic output

    if !unhandled.is_empty() {
        panic!(
            "FLAT ENUM ROUTING SAFETY VIOLATION:\n\
            The following `Message` variants are defined but not explicitly handled in any sub-router:\n\
            {:#?}\n\
            \n\
            Did you add a new Message variant but forget to match it in the app module?",
            unhandled
        );
    }
}
