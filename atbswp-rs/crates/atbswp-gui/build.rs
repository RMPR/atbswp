fn main() {
    // Fixed light colours -> pin the light widget style (dark widgets on a
    // white background render as invisible text).  Translations from
    // lang/<code>/LC_MESSAGES/atbswp-gui.po are compiled into the binary and
    // switched at run time with slint::select_bundled_translation.
    // Plain msgid lookup (no per-component msgctxt), matching lang/*.po.
    let config = slint_build::CompilerConfiguration::new()
        .with_style("fluent-light".into())
        .with_default_translation_context(slint_build::DefaultTranslationContext::None)
        .with_bundled_translations("lang");
    println!("cargo:rerun-if-changed=lang");
    slint_build::compile_with_config("ui/app.slint", config).expect("slint build failed");
}
