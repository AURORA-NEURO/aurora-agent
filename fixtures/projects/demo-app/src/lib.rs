pub fn seam() -> &'static str {
    "widget-gadget seam"
}

#[test]
fn the_seam_names_both_parts() {
    assert!(seam().contains("widget"));
}
