#![allow(dead_code)]

extern crate stm32f7_discovery as stm32f7;
extern crate alloc;

use stm32f7::lcd;
use stm32f7::lcd::Color;
use stm32f7::lcd::font::FontRenderer;
use alloc::boxed::Box;

const WIDTH: i32 = 480;
const HEIGHT: i32 = 272;

const PIXEL_BUFFER_SIZE: usize = 2000;

pub struct Renderer<'a, T: lcd::Framebuffer + 'a> {
    pixel_markers: [u32; ((WIDTH * HEIGHT + 31) / 32) as usize],
    drawn_pixels_x: [i16; 2 * PIXEL_BUFFER_SIZE],
    drawn_pixels_y: [i16; 2 * PIXEL_BUFFER_SIZE],
    drawn_pixel_count: [usize; 2],
    current_buffer: u8,
    layer: &'a mut lcd::Layer<T>,
    direct: bool,
    frame_counter: i32,
    portrait: bool,
    width: i32,
    height: i32,
    bg_func: Box<FnMut(i32, i32) -> Color>
}

impl<'a, T: lcd::Framebuffer> Renderer<'a, T> {
    pub fn new(l: &'a mut lcd::Layer<T>, background: Box<FnMut(i32, i32) -> Color>) -> Renderer<T> {
        Renderer {
            pixel_markers: [0; ((WIDTH * HEIGHT + 31) / 32) as usize],
            drawn_pixels_x: [0; 2 * PIXEL_BUFFER_SIZE],
            drawn_pixels_y: [0; 2 * PIXEL_BUFFER_SIZE],
            drawn_pixel_count: [0; 2],
            current_buffer: 0,
            layer: l,
            direct: true,
            frame_counter: 0,
            portrait: false,
            width: WIDTH,
            height: HEIGHT,
            bg_func: background
        }
    }

    pub fn set_bg(&mut self, func: Box<FnMut(i32, i32) -> Color>) {
        self.bg_func = func;
    }

    fn mark_pixel(&mut self, x: i32, y: i32, state: bool) {
        let index = x + y * WIDTH;
        let mask = 1 << (index % 32);
        if state {
            self.pixel_markers[(index / 32) as usize] |= mask;
        } else {
            self.pixel_markers[(index / 32) as usize] &= !mask;
        }
    }

    fn is_pixel_marked(&mut self, x: i32, y: i32) -> bool {
        let index = x + y * WIDTH;
        let mask = 1 << (index % 32);
        let marker = self.pixel_markers[(index / 32) as usize];
        marker & mask != 0
    }

    pub fn set_pixel(&mut self, px: i32, py: i32, color: Color) {
        let mut x = px;
        let mut y = py;

        if self.portrait {
            x = WIDTH - py;
            y = px;
        }

        if x < 0 || x >= WIDTH || y < 0 || y >= HEIGHT {
            return;
        }

        if self.direct {
            self.layer
                .print_point_color_at(x as usize, y as usize, color);
        } else {
            self.layer
                .print_point_color_at(x as usize, y as usize, color);

            self.mark_pixel(x, y, true);

            let offset = self.current_buffer as usize * PIXEL_BUFFER_SIZE;
            let index = self.drawn_pixel_count[self.current_buffer as usize] + offset;
            self.drawn_pixels_x[index] = x as i16;
            self.drawn_pixels_y[index] = y as i16;
            self.drawn_pixel_count[self.current_buffer as usize] += 1;
        }
    }

    pub fn begin_frame(&mut self) {
        let last_buffer = 1 - self.current_buffer;
        let offset = last_buffer as usize * PIXEL_BUFFER_SIZE;
        let size = self.drawn_pixel_count[last_buffer as usize];
        for i in 0..size {
            let x = self.drawn_pixels_x[(i + offset) as usize] as i32;
            let y = self.drawn_pixels_y[(i + offset) as usize] as i32;
            self.mark_pixel(x, y, false);
        }
        self.drawn_pixel_count[self.current_buffer as usize] = 0;
        self.direct = false;
    }

    pub fn end_frame(&mut self) {
        let last_buffer = 1 - self.current_buffer;
        let offset = last_buffer as usize * PIXEL_BUFFER_SIZE;
        let size = self.drawn_pixel_count[last_buffer as usize];
        for i in 0..size {
            let x = self.drawn_pixels_x[(i + offset) as usize] as i32;
            let y = self.drawn_pixels_y[(i + offset) as usize] as i32;
            if !self.is_pixel_marked(x, y) {
                let color = self.get_background(x, y);
                self.layer.print_point_color_at(x as usize, y as usize, color);
            }
        }

        self.current_buffer = last_buffer;
        self.frame_counter += 1;
        self.direct = true;
    }

    pub fn flush(&mut self) {
        for buf in 0..2 {
            let offset = buf as usize * PIXEL_BUFFER_SIZE;
            let size = self.drawn_pixel_count[buf as usize];
            for i in 0..size {
                let x = self.drawn_pixels_x[(i + offset) as usize] as i32;
                let y = self.drawn_pixels_y[(i + offset) as usize] as i32;
                let color = self.get_background(x, y);
                self.layer
                    .print_point_color_at(x as usize, y as usize, color);
                self.mark_pixel(x, y, false);
            }
            self.drawn_pixel_count[buf as usize] = 0;
        }
    }

    pub fn clear(&mut self) {
        self.flush();
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let color = self.get_background(x, y);
                self.layer
                    .print_point_color_at(x as usize, y as usize, color);
            }
        }
    }

    pub fn clear_area(&mut self, x: i32, y: i32, w: i32, h: i32) {
        if self.portrait {
            self.clear_area_landscape(WIDTH - y - h, x, h, w);
        } else {
            self.clear_area_landscape(x, y, w, h);
        }
    }

    fn clear_area_landscape(&mut self, x: i32, y: i32, w: i32, h: i32) {
        for py in y..y + h {
            for px in x..x + w {
                let color = self.get_background(px, py);
                self.layer
                    .print_point_color_at(px as usize, py as usize, color);
            }
        }
    }

    pub fn get_background(&mut self, px: i32, py: i32) -> Color {
        let mut x = px;
        let mut y = py;
        if self.portrait {
            x = py;
            y = WIDTH - px;
        }
        (self.bg_func)(x, y)
    }

    pub fn set_portrait(&mut self, state: bool) {
        self.portrait = state;
        if state {
            self.width = HEIGHT;
            self.height = WIDTH;
        } else {
            self.width = WIDTH;
            self.height = HEIGHT;
        }
    }

    pub fn get_width(&self) -> i32 {
        return self.width;
    }

    pub fn get_height(&self) -> i32 {
        return self.height;
    }

    pub fn draw_block_3d(&mut self, x: i32, y: i32, width: i32, height: i32, depth: i32, color: Color) {
        self.draw_line(x, y, x + width, y + width / 2, color);
        self.draw_line(x + width, y + width / 2, x + width + depth, y + width / 2 - depth / 2, color);
        self.draw_line(x, y, x + depth, y - depth / 2, color);
        self.draw_line(x + depth, y - depth / 2, x + width + depth, y + width / 2 - depth / 2, color);

        self.draw_line(x, y + height, x + width, y + width / 2 + height, color);
        self.draw_line(x + width, y + width / 2 + height, x + width + depth, y + width / 2 - depth / 2 + height, color);

        self.draw_line(x, y, x, y + height, color);
        self.draw_line(x + width - 1, y + width / 2, x + width - 1, y + width / 2 + height, color);
        self.draw_line(x + width + depth, y + width / 2 - depth / 2, x + width + depth, y + width / 2 - depth / 2 + height, color);
    }

    pub fn draw_block_3d_solid(&mut self, x: i32, y: i32, width: i32, height: i32, depth: i32, left_color: Color, right_color: Color, top_color: Color) {
        if depth > width {
            self.draw_y_oblique(x, y, depth, 1, depth / 2, -depth / 2, top_color);
            self.draw_y_oblique(x + depth, y - depth / 2, width, depth / 2, 1, width / 2, top_color);
            self.draw_y_oblique(x, y, width, 1, width / 2, 0, top_color);
            self.draw_y_oblique(x + width, y, depth, width / 2, 1, width / 2 -depth / 2, top_color);
        } else {
            self.draw_y_oblique(x, y, depth, 1, width / 2, -depth / 2, top_color);
            self.draw_y_oblique(x + depth, y - depth / 2, width, width / 2, 1, width / 2, top_color);
            self.draw_y_oblique(x, y, width, 1, depth / 2, width / 2 - depth / 2, top_color);
            self.draw_y_oblique(x + width, y + width / 2 - depth / 2, depth, depth / 2, 1, 0, top_color);
        }
        
        self.draw_y_oblique(x, y, width, height, height, width / 2, left_color);
        self.draw_y_oblique(x + width, y + width / 2, depth, height, height, -depth / 2, right_color);
    }

    pub fn draw_y_oblique(&mut self, x: i32, y:i32, width: i32, height0: i32, height1: i32, y_movement: i32, color: Color) {
        for i in 0..width {
            let base_y = y + i * y_movement / width;
            let h = (height1 - height0) * i / (width - 1) + height0;
            for j in 0..h {
                self.set_pixel(x + i, base_y + j, color);
            }
        }
    }

    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        if y0 == y1 {
            for px in x0..=x1 {
                self.set_pixel(px, y0, color);
            }
        } else if x0 == x1 {
            for py in y0..=y1 {
                self.set_pixel(x0, py, color)
            }
        } else {
            let d_x = x1 - x0;
            let d_y = y1 - y0;
            let mut d_err = d_y as f32 / d_x as f32;
            if d_err < 0f32 {
                d_err = -d_err;
            }
            let mut error = 0f32;
            let mut y = y0;
            let mut sign_d_x = 1;
            let mut abs_d_x = d_x;
            if abs_d_x < 0 {
                abs_d_x = -abs_d_x;
                sign_d_x = -1;
            }
            for i in 0..=abs_d_x {
                let x = x0 + sign_d_x * i;
                self.set_pixel(x, y, color);
                error = error + d_err;
                while error >= 0.5f32 {
                    if d_y > 0 {
                        y += 1;
                    } else if d_y < 0 {
                        y -= 1;
                    }
                    if error >= 1.5f32 {
                        self.set_pixel(x, y, color);
                    }
                    error = error - 1f32;
                }
            }
        }
    }

    pub fn draw_rect_solid(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        for py in y..y + h {
            for px in x..x + w {
                self.set_pixel(px, py, color);
            }
        }
    }

    pub fn draw_rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        for px in x..x + w {
            self.set_pixel(px, y, color);
        }
        for py in y + 1..y + h - 1 {
            self.set_pixel(x, py, color);
        }
        for py in y + 1..y + h - 1 {
            self.set_pixel(x + w - 1, py, color);
        }
        for px in x..x + w {
            self.set_pixel(px, y + h - 1, color);
        }
    }

    pub fn draw_text(&mut self, font: &FontRenderer, text: &str, x: i32, y: i32, color: Color) {
        font.render(text, |px, py, v| {
            let alpha = (255f32 * v) as u8;
            let c = Color::rgba(color.red, color.green, color.blue, alpha);
            self.set_pixel(px as i32 + x, py as i32 + y, c)
        });
    }
}

pub fn weight_color(c: Color, w: f32) -> Color {
    Color::rgb((c.red as f32 * w) as u8, (c.green as f32 * w) as u8, (c.blue as f32 * w) as u8)
}

pub fn hsv_color(hue: f32, s: f32, v: f32) -> Color {
    let h = (hue as i32 % 360) as f32;

    let c = v * s;
    let x = (h as i32 % 120) as f32 / 60f32 - 1f32;
    let x = c * (1f32 - if x < 0f32 { -x } else { x });
    let m = v - c;

    let mut rgb = (0f32, 0f32, 0f32);
    if h < 60f32 {
        rgb.0 = c;
        rgb.1 = x;
    } else if h < 120f32 {
        rgb.0 = x;
        rgb.1 = c;
    } else if h < 180f32 {
        rgb.1 = c;
        rgb.2 = x;
    } else if h < 240f32 {
        rgb.1 = x;
        rgb.2 = c;
    } else if h < 300f32 {
        rgb.0 = x;
        rgb.2 = c;
    } else {
        rgb.0 = c;
        rgb.2 = x;
    }

    rgb.0 += m;
    rgb.1 += m;
    rgb.2 += m;

    lcd::Color::rgb(
        (255f32 * rgb.0) as u8,
        (255f32 * rgb.1) as u8,
        (255f32 * rgb.2) as u8,
    )
}
