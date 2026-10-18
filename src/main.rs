mod app;
mod browser;
mod catalog;
mod command;
mod dialogs;
mod document;
mod effects;
mod logging;
mod platform;
mod protocol;
mod resources;
mod state;
mod transforms;
mod transitions;
mod viewer;
mod viewport;
mod wake;

fn main() {
    logging::init();
    let window = dioxus::desktop::WindowBuilder::new()
        .with_title("Image Viewer")
        .with_inner_size(dioxus::desktop::LogicalSize::new(1024.0, 768.0))
        .with_min_inner_size(dioxus::desktop::LogicalSize::new(1024.0, 768.0))
        .with_position(dioxus::desktop::LogicalPosition::new(300.0, 100.0))
        .with_resizable(true)
        .with_decorations(true);
    let config = dioxus::desktop::Config::new()
        .with_window(window)
        .with_menu(platform::menu::native_menu())
        .with_disable_context_menu(true)
        .with_disable_drag_drop_handler(true)
        .with_asynchronous_custom_protocol("app", |_id, request, responder| {
            crate::wake::spawn_blocking(move || {
                responder.respond(crate::protocol::handle(&request));
            });
        })
        .with_custom_event_handler(|event, _target| {
            platform::launch::on_tao_event(event);
        });
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(app::app);
}
