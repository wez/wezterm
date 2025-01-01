use ::window::parameters::Border;
use ::window::ResizeIncrement;

pub struct ResizeIncrementCalculator {
    pub x: u16,
    pub y: u16,
    pub padding_left: usize,
    pub padding_top: usize,
    pub padding_right: usize,
    pub padding_bottom: usize,
    pub border: Border,
    pub tab_bar_height: usize,
}

impl Into<ResizeIncrement> for ResizeIncrementCalculator {
    fn into(self) -> ResizeIncrement {
        ResizeIncrement {
            x: self.x,
            y: self.y,
            base_width: (self.padding_left
                + self.padding_right
                + (self.border.left + self.border.right).get()) as u16,
            base_height: (self.padding_top
                + self.padding_bottom
                + (self.border.top + self.border.bottom).get()
                + self.tab_bar_height) as u16,
        }
    }
}
