use std::path::PathBuf;

use gtk::gdk::Display;
use gtk::prelude::*;
use gtk::{
    Application, ApplicationWindow, Box as GtkBox, Button, ContentFit, CssProvider, HeaderBar,
    Label, Orientation, Picture, ToggleButton,
};

const APP_ID: &str = "xyz.rmpr.atbswp";

fn image_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("atbswp")
        .join("img")
}

fn icon_picture(icon_file: &str) -> Picture {
    let image_path = image_root().join(icon_file);
    let image = Picture::for_filename(image_path);
    image.set_size_request(40, 40);
    image.set_can_shrink(true);
    image.set_content_fit(ContentFit::Contain);
    image
}

fn icon_button(icon_file: &str, tooltip: &str, action_name: &'static str) -> Button {
    let image = icon_picture(icon_file);
    let button = Button::new();
    button.set_child(Some(&image));
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("toolbar-button");
    button.connect_clicked(move |_| {
        println!("{action_name} clicked");
    });
    button
}

fn icon_toggle_button(icon_file: &str, tooltip: &str, action_name: &'static str) -> ToggleButton {
    let image = icon_picture(icon_file);
    let button = ToggleButton::new();
    button.set_child(Some(&image));
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("toolbar-button");
    button.connect_toggled(move |btn| {
        let state = if btn.is_active() { "on" } else { "off" };
        println!("{action_name} toggled {state}");
    });
    button
}

fn install_css() {
    let provider = CssProvider::new();
    provider.load_from_string(
        "
        .toolbar-button {
            border-radius: 0;
            border: 1px solid #d8d8d8;
            background: #ffffff;
            min-width: 72px;
            min-height: 72px;
            padding: 0;
        }

        .toolbar-button:hover {
            background: #f4f4f4;
        }

        .toolbar-button:active,
        .toolbar-button:checked {
            background: #dbe9ff;
            border-color: #7ea7e7;
        }

        .compact-header {
            min-height: 24px;
            padding-top: 0;
            padding-bottom: 0;
        }

        .compact-title {
            font-weight: 600;
        }
    ",
    );

    gtk::style_context_add_provider_for_display(
        &Display::default().expect("No display available"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn build_ui(app: &Application) {
    install_css();

    let window = ApplicationWindow::builder()
        .application(app)
        .title("atbswp")
        .resizable(false)
        .build();

    let title_label = Label::new(Some("atbswp"));
    title_label.add_css_class("compact-title");

    let header = HeaderBar::new();
    header.add_css_class("compact-header");
    header.set_title_widget(Some(&title_label));
    window.set_titlebar(Some(&header));

    let row = GtkBox::new(Orientation::Horizontal, 0);
    row.add_css_class("toolbar-row");

    row.append(&icon_button(
        "file-upload.png",
        "Load Capture",
        "Load Capture",
    ));
    row.append(&icon_button("save.png", "Save", "Save"));
    row.append(&icon_toggle_button(
        "video.png",
        "Start/Stop Capture",
        "Start/Stop Capture",
    ));
    row.append(&icon_toggle_button("play-circle.png", "Play", "Play"));
    row.append(&icon_button(
        "download.png",
        "Compile to executable",
        "Compile to executable",
    ));
    row.append(&icon_button("cog.png", "Preferences", "Preferences"));
    row.append(&icon_button("question-circle.png", "Help", "Help"));

    window.set_child(Some(&row));
    window.present();
}

fn main() {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_ui);
    app.run();
}
