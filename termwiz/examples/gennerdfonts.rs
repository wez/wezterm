//! This is used to generate a table in the wezterm docs

fn main() {
    println!("| | |");
    println!("|-|-|");
    for &(label, c) in termwiz::nerdfonts::NERD_FONT_GLYPHS {
        println!(
            "|<span class=\"nf big\">&#x{:x};</span>|{}|",
            c as u32, label
        );
    }
}
