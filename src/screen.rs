use limine::framebuffer::Framebuffer;

#[derive(Clone)]
pub struct Screen {
    framebuffer_addr: *mut u8,
    pub width: usize,
    pub height: usize,
    bytes_per_pixel: usize,
    bytes_per_row: usize,
}

/// safety: the pointer in framebuffer_addr needs to be unsafely derefrenced anyways
/// to be used, and the API always requires &mut to write to the pointer,
/// implying single ownership at the time of writing.
unsafe impl Send for Screen {}

/// RGB color
#[derive(Clone, Copy)]
pub struct Color(u32);

impl Color {
    pub fn red() -> Self {
        Self(0xFF0000)
    }

    pub fn green() -> Self {
        Self(0xFF00)
    }

    pub fn blue() -> Self {
        Self(0xFF)
    }

    pub fn white() -> Self {
        Self(0xFFFFFF)
    }

    pub fn black() -> Self {
        Self(0)
    }
}

impl Screen {
    /// Create a new screen from a framebuffer.
    /// ## Saftey
    /// the provided framebuffer must have valid information,
    /// and must live as long as the Screen lives.
    pub unsafe fn new(framebuffer: Framebuffer) -> Self {
        Self {
            framebuffer_addr: framebuffer.addr() as *mut _,
            bytes_per_pixel: (framebuffer.bpp() / 8) as usize,
            bytes_per_row: framebuffer.pitch() as usize,
            height: framebuffer.height() as usize,
            width: framebuffer.width() as usize,
        }
    }

    /// draw a single pixel on the screen.  
    /// Note: will panic if the position goes out of the screen.  
    /// i.e. assert!(x < self.width && y < self.height)
    pub fn draw_pixel(&mut self, x: usize, y: usize, color: Color) {
        assert!(x < self.width && y < self.height);
        let pixel_offset = x * self.bytes_per_pixel + y * self.bytes_per_row;
        unsafe {
            self.write_pixel(pixel_offset, color);
        }
    }

    /// write a single pixel to the framebuffer
    /// ## Saftey
    /// Ensure that the offset is valid. This does not check it.
    /// Takes &mut self to ensure ownership of the Screen.
    #[inline]
    unsafe fn write_pixel(&mut self, offset: usize, color: Color) {
        unsafe {
            self.framebuffer_addr
                .add(offset)
                .cast::<u32>()
                .write(color.0);
        }
    }

    /// Paint all the pixels at once
    pub fn draw_all(&mut self, color: Color) {
        for y in 0..self.height {
            let mut offset = y * self.bytes_per_row;
            for _ in 0..self.width {
                unsafe { self.write_pixel(offset, color) };
                offset += self.bytes_per_pixel;
            }
        }
    }
}
