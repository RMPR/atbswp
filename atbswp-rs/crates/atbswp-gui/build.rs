fn main() {
    // The windows use fixed light colours, so pin the light widget style
    // instead of following the desktop theme (dark widgets on a white
    // background render as invisible text).
    let config = slint_build::CompilerConfiguration::new().with_style("fluent-light".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("slint build failed");
}
