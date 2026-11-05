mod appearance;
mod demo;
mod menus;
mod project_tree;
mod search;
mod workspace;
use gpui_kit::*;
use workspace::Workspace;

actions!(diana, [Quit]);

fn main() {
    application().with_assets(assets::AllAssets).run(|cx| {
        init(cx);
        component::set_locale("en");
        appearance::configure(cx);
        workspace::bind_keys(cx);
        let quit_key = if cfg!(target_os = "macos") {
            "cmd-q"
        } else {
            "ctrl-q"
        };
        cx.bind_keys([KeyBinding::new(quit_key, Quit, None)]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus([Menu::new("Diana").items([MenuItem::action("Quit Diana", Quit)])]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        let chrome = if cfg!(target_os = "macos") {
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("Diana".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }
        } else {
            component::TitleBar::window_options()
        };
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(900.), px(620.))),
            ..chrome
        };
        match open_window(options, cx, |window, cx| {
            cx.new(|cx| Workspace::new(window, cx))
        }) {
            Ok(_) => cx.activate(true),
            Err(error) => {
                eprintln!("Cannot open Diana window: {error}");
                cx.quit();
            }
        }
    });
}
