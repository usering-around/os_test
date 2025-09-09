use core::{
    fmt,
    ops::{Deref, DerefMut},
};

use spin::mutex::SpinMutex;

use crate::screen::{Color, Screen};

/// The width of one character in pixels
const CHAR_WIDTH: usize = 8;

/// the height of one character in pixels
const CHAR_HEIGHT: usize = 16;
// space between characters in pixels
const SPACE_BETWEEN_CHARS: usize = 1;

/// A thread-unsafe console abstracton on a SCREEN which can draw ascii characters.  
/// It starts drawing characters from upwards to downwards, if it reaches the end of a line it simply continues to the next line
/// and if it reaches the end of the screen, it simply continues from the first line.
/// This struct implements fmt::Write, use it for writing multiple characters.
#[derive(Clone)]
pub struct Console {
    /// the screen to draw characters on
    screen: Screen,
    /// the current position of the cursor (which represents the next place to draw the character)
    cursor_pos: (usize, usize),
    /// the color of the background
    pub bg_color: Color,
    /// the color of the foreground (text)
    pub fg_color: Color,
}

impl Console {
    /// Get a new console from a screen.  
    /// Note: immediately colors the whole screen to bg_color.
    pub fn new(mut screen: Screen, bg_color: Color, fg_color: Color) -> Self {
        screen.draw_all(bg_color);
        Self {
            screen,
            cursor_pos: (0, 0),
            bg_color,
            fg_color,
        }
    }

    /// print a single ascii character to the console with the default colors
    pub fn print_char(&mut self, c: u8) {
        self.print_char_colored(c, self.fg_color, self.bg_color);
    }
    /// Print a single ascii character to the console with specific colors
    pub fn print_char_colored(&mut self, c: u8, fg_color: Color, bg_color: Color) {
        let (mut x, mut y) = self.cursor_pos;
        if c == b'\n' {
            x = 0;
            y += CHAR_HEIGHT;
            if y + CHAR_HEIGHT > self.screen.height {
                y = 0;
            }
            self.cursor_pos = (x, y);
            return;
        }
        self.draw_char(c, x, y, fg_color, bg_color);

        // increment cursor, + 1 for space between characters
        x += CHAR_WIDTH + SPACE_BETWEEN_CHARS;
        // we need to make sure there is enough place for the next character
        if x + CHAR_WIDTH > self.screen.width {
            // we go to the next line
            x = 0;
            y += CHAR_HEIGHT;
            if y + CHAR_HEIGHT > self.screen.height {
                // we go to the first line
                y = 0;
            }
        }
        self.cursor_pos = (x, y);
    }

    /// Clear the console, painting it in the pre-assigned background color
    pub fn clear(&mut self) {
        self.cursor_pos = (0, 0);
        self.screen.draw_all(self.bg_color);
    }

    /// Draw a single ascii character to the console
    // TODO: perhaps make the font and the related constants fields of the Console to generalize to more fonts?
    fn draw_char(&mut self, c: u8, x: usize, mut y: usize, fg_color: Color, bg_color: Color) {
        let font_bytes = include_bytes!("/home/makeitrain/Downloads/AIXOID9.F16");
        let mut pos = (c as usize) * CHAR_HEIGHT;
        for _ in 0..CHAR_HEIGHT {
            let display_byte = font_bytes[pos];
            // now we inspect each bit and draw accoridngly
            // possible optimization: have a table which maps bytes to array of bitfields
            for i in 0..CHAR_WIDTH {
                let is_set = display_byte & (1 << i) != 0;
                if is_set {
                    // we have to draw fg color
                    self.screen.draw_pixel(x + (CHAR_WIDTH - i), y, fg_color);
                } else {
                    self.screen.draw_pixel(x + (CHAR_WIDTH - i), y, bg_color);
                }
            }
            y += 1;
            pos += 1;
        }
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for char in s.chars() {
            if let Some(ascii) = char.as_ascii() {
                self.print_char(ascii.to_u8())
            }
        }
        Ok(())
    }
}

/// thread safe console. This type exists to provide a Write implementation for SpinMutex<Console>.
pub struct ThreadSafeConsole(SpinMutex<Console>);

impl ThreadSafeConsole {
    pub fn new(console: Console) -> Self {
        ThreadSafeConsole(SpinMutex::new(console))
    }
}
impl Deref for ThreadSafeConsole {
    type Target = SpinMutex<Console>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ThreadSafeConsole {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// writeln implementation for GlobalConsole
impl fmt::Write for ThreadSafeConsole {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut console = self.0.lock();
        console.write_str(s)
    }
}
