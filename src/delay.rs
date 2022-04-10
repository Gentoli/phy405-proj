use embedded_hal::delay::blocking::DelayUs;

use cortex_m::asm::delay as cycle_delay;
use wio_terminal::hal::ehal::blocking::delay::DelayMs;

// use wio::prelude::*;
use wio_terminal::hal::time::{Nanoseconds, U32Ext};

pub(crate) struct InstDelay;

fn us_to_cycle(us: u32) -> u32 {
    static PERIOD: Nanoseconds = Nanoseconds(10);
    let delay: Nanoseconds = (us as u32).us().into();
    delay.0 / PERIOD.0
}

impl DelayMs<u16> for InstDelay {
    fn delay_ms(&mut self, ms: u16) {
        cycle_delay_ms(ms as u32);
    }
}

impl DelayUs for InstDelay {
    type Error = ();

    fn delay_us(&mut self, us: u32) -> Result<(), Self::Error> {
        cycle_delay_us(us);
        Ok(())
    }
}

pub fn cycle_delay_ms(ms: u32) {
    // Use 100MHz instead of 120MHz to have at least `ms` delay
    cycle_delay_us(ms * 1000);
}

pub fn cycle_delay_us(us: u32) {
    // Use 100MHz instead of 120MHz to have at least `ms` delay
    cycle_delay(us_to_cycle(us));
}