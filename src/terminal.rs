use core::ops::Add;
use embedded_graphics as eg;

use eg::mono_font::{ascii::FONT_6X12, MonoTextStyle};
use eg::pixelcolor::Rgb565;
use eg::prelude::*;
use eg::primitives::{PrimitiveStyleBuilder, Rectangle};
use eg::text::Text;
use embedded_graphics::mono_font::ascii::{FONT_8X13, FONT_9X15};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::text::TextStyle;
use embedded_text::alignment::{HorizontalAlignment, VerticalAlignment};
use embedded_text::style::TextBoxStyleBuilder;
use embedded_text::TextBox;

use wio_terminal::{Scroller, LCD};

// From https://github.com/atsamd-rs/atsamd/blob/0c241f395e63ee25eb41984d703e4babdae454c2/boards/wio_terminal/examples/usb_serial_display.rs
// By @jbeaurivage
type TextSegment = ([u8; 32], usize);

pub struct Terminal {
    // text_style: MonoTextStyle<'a, Rgb565>,
    cursor: Point,
    display: LCD,
    scroller: Scroller,
}

impl Terminal {
    pub fn new(mut display: LCD) -> Self {
        // Clear the screen.
        let style = PrimitiveStyleBuilder::new()
            .fill_color(Rgb565::BLACK)
            .build();
        let backdrop =
            Rectangle::with_corners(Point::new(0, 0), Point::new(320, 320)).into_styled(style);
        backdrop.draw(&mut display).ok().unwrap();

        let scroller = display.configure_vertical_scroll(0, 0).unwrap();

        Self {
            // text_style: MonoTextStyle::new(&FONT_6X12, Rgb565::WHITE),
            cursor: Point::new(0, 8),
            display,
            scroller,
        }
    }

    pub fn write_str(&mut self, str: &str) {
        for character in str.chars() {
            self.write_character(character);
        }
    }

    pub fn write_pos(&mut self, pos: Point, str: &str) {
        let filled_background = MonoTextStyleBuilder::new()
            .font(&FONT_8X13)
            .text_color(Rgb565::YELLOW)
            .background_color(Rgb565::BLUE)
            .build();

        Text::new(
            str,
            pos,
            filled_background,
        )
            .draw(&mut self.display).unwrap();
        // Rectangle::with_corners(
        //     pos,
        //     pos.add(Point::new(20, 40)),
        // )
        //     .into_styled(
        //         PrimitiveStyleBuilder::new()
        //             .fill_color(Rgb565::BLACK)
        //             .build(),
        //     )
        //     .draw(&mut self.display)
        //     .ok()
        //     .unwrap();
        // Text::new(str, pos, MonoTextStyle::new(&FONT_6X12, Rgb565::WHITE))
        //     .draw(&mut self.display)
        //     .ok()
        //     .unwrap();
    }

    pub fn write_character(&mut self, c: char) {
        if self.cursor.x >= 320 || c == '\n' {
            self.cursor = Point::new(0, self.cursor.y + FONT_6X12.character_size.height as i32);
        }
        if self.cursor.y >= 240 {
            self.animate_clear();
            self.cursor = Point::new(0, 0);
        }

        if c != '\n' {
            let mut buf = [0u8; 8];

            Text::new(
                c.encode_utf8(&mut buf),
                self.cursor,
                MonoTextStyle::new(&FONT_6X12, Rgb565::WHITE),
            )
                .draw(&mut self.display)
                .ok()
                .unwrap();

            self.cursor.x += (FONT_6X12.character_size.width + FONT_6X12.character_spacing) as i32;
        }
    }

    pub fn write(&mut self, segment: TextSegment) {
        let (buf, count) = segment;
        for (i, character) in buf.iter().enumerate() {
            if i >= count {
                break;
            }
            self.write_character(*character as char);
        }
    }

    fn animate_clear(&mut self) {
        for x in (0..320).step_by(FONT_6X12.character_size.width as usize) {
            self.display
                .scroll_vertically(&mut self.scroller, FONT_6X12.character_size.width as u16)
                .ok()
                .unwrap();
            Rectangle::with_corners(
                Point::new(x, 0),
                Point::new(x + FONT_6X12.character_size.width as i32, 240),
            )
                .into_styled(
                    PrimitiveStyleBuilder::new()
                        .fill_color(Rgb565::BLACK)
                        .build(),
                )
                .draw(&mut self.display)
                .ok()
                .unwrap();

            for _ in 0..1000 {
                cortex_m::asm::nop();
            }
        }
    }
}