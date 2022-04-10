use wio_terminal::aliases::{DAC0Reset, DAC1Reset};
use wio_terminal::hal::adc::Adc;
use wio_terminal::hal::clock::GenericClockController;
use wio_terminal::hal::gpio::{Alternate, B, Pin};
use wio_terminal::pac::{DAC, MCLK};
use wio_terminal::pac::gclk::genctrl::SRC_A::DFLL;
use wio_terminal::pac::gclk::pchctrl::GEN_A;
use wio_terminal::hal::ehal::digital::v2::OutputPin;
use crate::delay::cycle_delay_ms;
use crate::state::OutputValues;

pub struct Dac {
    dac: DAC,
    dac0_value: u16,
    dac1_value: u16,
}

impl Dac {
    pub fn new(
        dac: DAC,
        dac0_pin: DAC0Reset,
        dac1_pin: DAC1Reset,
        mclk: &mut MCLK,
        clocks: &mut GenericClockController,
        gclk: GEN_A,
    ) -> Self {
        mclk.apbdmask.modify(|_, w| w.dac_().set_bit());
        let dac_clock = clocks.configure_gclk_divider_and_source(gclk, 1, DFLL, false)
            .expect("dac clock setup failed");
        clocks.dac(&dac_clock).expect("dac clock setup failed");

        dac.ctrla.modify(|_, w| w.swrst().set_bit());

        while dac.ctrla.read().swrst().bit_is_set() || dac.syncbusy.read().swrst().bit_is_set() {}

        dac.dacctrl.iter()
            .for_each(|ctrl| ctrl.modify(|_, w|
                w.enable().set_bit()
                    .refresh().refresh_1()
            ));

        dac.ctrla.modify(|_, w| w.enable().set_bit());

        while dac.syncbusy.read().enable().bit_is_set() {}
        dac0_pin.into_alternate::<B>();
        dac1_pin.into_alternate::<B>();

        while dac.status.read().ready0().bit_is_clear() || dac.status.read().ready1().bit_is_clear() {}

        Self {
            dac,
            dac0_value: 0,
            dac1_value: 0,
        }
    }

    pub fn update(&self) {
        self.dac.data[0].write(|w| unsafe { w.data().bits(self.dac0_value) });
        self.dac.data[1].write(|w| unsafe { w.data().bits(self.dac1_value) });
        while self.dac.status.read().eoc0().bit_is_clear() || self.dac.status.read().eoc1().bit_is_clear() {}
    }

    pub fn set_adc0_desired(&mut self, value: u16) {
        self.dac0_value = value;
    }

    pub fn set_adc1_desired(&mut self, value: u16) {
        self.dac1_value = value;
    }

    pub fn set(&mut self, dac0: u16, dac1: u16) {
        self.dac0_value = dac0;
        self.dac1_value = dac1;
    }

    pub fn set_output(&mut self, output: &OutputValues) {
        self.dac0_value = output.dac0;
        self.dac1_value = output.dac1;
    }
}
