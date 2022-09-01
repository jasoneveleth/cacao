//! This example showcases the TextView

use cacao::textview::{TextView, TextViewDelegate};
use cacao::layout::{Layout, LayoutConstraint};
use cacao::view::View;

use cacao::appkit::menu::{Menu, MenuItem};
use cacao::appkit::window::{Window, WindowConfig, WindowDelegate};
use cacao::appkit::{App, AppDelegate};

struct BasicApp {
    window: Window<AppWindow>
}

impl AppDelegate for BasicApp {
    fn did_finish_launching(&self) {
        App::set_menu(vec![
            Menu::new("", vec![
                MenuItem::Services,
                MenuItem::Separator,
                MenuItem::Hide,
                MenuItem::HideOthers,
                MenuItem::ShowAll,
                MenuItem::Separator,
                MenuItem::Quit,
            ]),
            Menu::new("File", vec![MenuItem::CloseWindow]),
            Menu::new("Edit", vec![
                MenuItem::Undo,
                MenuItem::Redo,
                MenuItem::Separator,
                MenuItem::Cut,
                MenuItem::Copy,
                MenuItem::Paste,
                MenuItem::Separator,
                MenuItem::SelectAll,
            ]),
            Menu::new("View", vec![MenuItem::EnterFullScreen]),
            Menu::new("Window", vec![
                MenuItem::Minimize,
                MenuItem::Zoom,
                MenuItem::Separator,
                MenuItem::new("Bring All to Front"),
            ]),
            Menu::new("Help", vec![]),
        ]);

        App::activate();
        self.window.show();
    }

    fn should_terminate_after_last_window_closed(&self) -> bool {
        true
    }
}

#[derive(Debug, Default)]
pub struct ConsoleLogger;

impl TextViewDelegate for ConsoleLogger {
    const NAME: &'static str = "ConsoleLogger";

    fn text_did_change(&self, value: &str) {
        println!("Did change to: {}", value);
    }
}

#[derive(Debug)]
struct AppWindow {
    input: TextView<ConsoleLogger>,
    content: View
}

impl AppWindow {
    pub fn new() -> Self {
        AppWindow {
            input: TextView::with(ConsoleLogger),
            content: View::new()
        }
    }
}

impl WindowDelegate for AppWindow {
    const NAME: &'static str = "WindowDelegate";

    fn did_load(&mut self, window: Window) {
        window.set_title("TextView Example");
        window.set_minimum_content_size(480., 270.);

        self.content.add_subview(&self.input);
        window.set_content_view(&self.content);

        LayoutConstraint::activate(&[
            self.content.leading.constraint_equal_to(&self.content.leading),
            self.content.trailing.constraint_equal_to(&self.content.trailing),
            self.content.top.constraint_equal_to(&self.content.top),
            self.content.bottom.constraint_equal_to(&self.content.bottom),
        ]);
    }
}

fn main() {
    App::new("com.test.window", BasicApp {
        window: Window::with(WindowConfig::default(), AppWindow::new())
    })
    .run();
}
