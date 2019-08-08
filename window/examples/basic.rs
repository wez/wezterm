use failure::Fallible;

fn main() -> Fallible<()> {
    let win = window::os::windows::window::Window::new_window("myclass", "the title", 800, 600)?;

    win.show();

    window::os::windows::window::run_message_loop()
}
