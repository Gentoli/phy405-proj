#![no_std]
#![no_main]
#![feature(let_else)]

mod terminal;
mod delay;
mod state;
mod dac;
mod logics;

use panic_halt as _;
use wio_terminal as wio;

use wio::entry;
use wio::hal::clock::GenericClockController;
use wio::hal::delay::Delay;
use wio::pac::{CorePeripherals, Peripherals};
use wio::prelude::*;

#[rtic::app(device = wio_terminal::pac, peripherals = true,
dispatchers = [
DMAC_0,
DMAC_1,
DMAC_2,
DMAC_3,
DMAC_OTHER,
SERCOM0_0,
SERCOM0_1,
SERCOM0_2,
SERCOM0_OTHER,
SERCOM1_0,
SERCOM1_1,
SERCOM1_2,
SERCOM1_OTHER,
])]
mod app {
    use cortex_m::interrupt;
    use wio::prelude::*;
    use wio_terminal as wio;

    // Time
    // use rtic::cyccnt::{Instant, U32Ext};
    use wio::hal::clock::GenericClockController;
    use systick_monotonic::*;

    // IO
    use wio::hal::gpio::*;
    use wio::{Pins, Sets};
    use wio::aliases::{LcdBacklight, UserLed};

    // ADC
    // crate
    use crate::delay::InstDelay;
    // use crate::strings::str_to_fixed as stf;
    use crate::terminal::Terminal;
    use wio::hal::adc::{Adc, Resolution, FreeRunning, InterruptAdc, Reference, SampleRate};
    use wio::pac::{ADC0, ADC1};
    use wio::hal::pac::gclk::pchctrl::GEN_A::{GCLK9, GCLK10, GCLK11};

    // Buttons
    use wio::{Button, ButtonEvent};
    use wio_terminal::ButtonController;

    // use crate::descriptors::{KeyboardNkroReport, KeyboardNkroReportOut};
    use arrayvec::ArrayString;
    use core::fmt::Write;
    // use heapless::consts::*;
    // use heapless::spsc::{Consumer, MultiCore, Producer, Queue, SingleCore};
    use wio_terminal::hal::qspi::Command::Read;
    use wio_terminal::hal::rtc::*;
    use ssmarshal::{deserialize, serialize};
    use cortex_m::asm::nop;
    use embedded_graphics::geometry::Point;
    use rtic::Mutex;
    use wio_terminal::hal::time::Hertz;
    // use nb::block;
    use crate::dac::Dac;
    use crate::logics::State;

    #[shared]
    struct Resources {
        // Buttons
        #[lock_free]
        button_ctr: ButtonController,

        // Data
        #[lock_free]
        inputs: crate::state::InputValues,
        #[lock_free]
        outputs: crate::state::OutputValues,
        #[lock_free]
        desired_out: crate::logics::DesiredOutput,
        #[lock_free]
        state: crate::logics::State,
    }

    #[local]
    struct Local {
        terminal: Terminal,
        backlight: LcdBacklight,
        backlight_state: bool,

        user_led: UserLed,

        i_adc0: InterruptAdc<ADC0, FreeRunning>,
        i_adc1: InterruptAdc<ADC1, FreeRunning>,

        // ADC
        dac: Dac,
    }

    #[monotonic(binds = SysTick, default = true)]
    type SysTickMonotonic = Systick<100>;

    const PERIOD: u32 = 16_000_000;

    #[init]
    fn init(cx: init::Context) -> (Resources, Local, init::Monotonics) {
        let mut core = cx.core;
        core.DWT.enable_cycle_counter();

        let mut device = cx.device;

        let mut clocks = GenericClockController::with_external_32kosc(
            device.GCLK,
            &mut device.MCLK,
            &mut device.OSC32KCTRL,
            &mut device.OSCCTRL,
            &mut device.NVMCTRL,
        );

        device.OSC32KCTRL.rtcctrl.write(|w| w.rtcsel().xosc32k());

        let gclk = clocks.gclk0();
        let freq: Hertz = gclk.into();
        let systick = Systick::new(core.SYST, freq.0);
        // PORT
        let mut sets: Sets = Pins::new(device.PORT).split();

        // Blue Led
        let mut user_led = sets.user_led.into_push_pull_output();

        // LCD

        // Initialize the ILI9341-based LCD display. Create a black backdrop the size of
        // the screen.
        let (display, backlight) = sets
            .display
            .init(
                &mut clocks,
                device.SERCOM7,
                &mut device.MCLK,
                58.mhz(),
                &mut InstDelay {},
            )
            .unwrap();
        let mut term = Terminal::new(display);

        term.write_str("Hello World! -----------------------------------\n");

        // ADC
        let mut header_pins = sets.header_pins;

        let mut adc0 = Adc::adc0(device.ADC0, &mut device.MCLK, &mut clocks, GCLK10);
        let mut adc1 = Adc::adc1(device.ADC1, &mut device.MCLK, &mut clocks, GCLK11);
        adc0.samples(SampleRate::_256);
        adc0.resolution(Resolution::_16BIT);
        adc1.samples(SampleRate::_256);
        adc1.resolution(Resolution::_12BIT);
        let mut a0_d0: Pin<PB08, Alternate<B>> = header_pins.a0_d0.into();
        let mut a1_d1: Pin<PB09, Alternate<B>> = header_pins.a1_d1.into();

        let mut i_adc0: InterruptAdc<_, FreeRunning> = InterruptAdc::from(adc0);
        let mut i_adc1: InterruptAdc<_, FreeRunning> = InterruptAdc::from(adc1);
        i_adc0.start_conversion(&mut a0_d0);
        i_adc1.start_conversion(&mut a1_d1);

        // DAC
        let dac = Dac::new(device.DAC, header_pins.dac0, header_pins.dac1, &mut device.MCLK, &mut clocks, GCLK9);

        user_led.set_low().unwrap();

        // Buttons
        let button_ctr =
            sets.buttons
                .init(device.EIC, &mut clocks, &mut device.MCLK);

        // Start Tasks
        blinky::spawn().unwrap();
        print_state::spawn().unwrap();
        sync::spawn().unwrap();
        dac_update::spawn().unwrap();

        (Resources {
            button_ctr,
            inputs: Default::default(),
            outputs: Default::default(),
            desired_out: Default::default(),
            state: Default::default(),
        }, Local {
            terminal: term,
            backlight,
            backlight_state: true,
            user_led,
            i_adc0,
            i_adc1,
            dac,
        }, init::Monotonics(systick))
    }

    #[task(local = [user_led])] // ,d7
    fn blinky(cx: blinky::Context) {
        cx.local.user_led.toggle();
        blinky::spawn_after(200.millis()).unwrap();
    }

    #[task(local = [terminal], capacity = 6)]
    fn print(cx: print::Context, msg: ArrayString<[u8; 256]>, pos: Point) {
        // cx.local.terminal.write_str(&msg[..]);
        cx.local.terminal.write_pos(pos, &msg[..]);
    }

    #[task(shared = [inputs, outputs, desired_out, state])]
    fn sync(mut cx: sync::Context) {
        let state = State::from(&cx.shared.inputs, &cx.shared.desired_out);

        *cx.shared.outputs = state.get_output_level();
        *cx.shared.state = state;

        sync::spawn_after(10.millis()).unwrap();
    }

    #[task(local = [dac], shared = [outputs])]
    fn dac_update(cx: dac_update::Context) {
        cx.local.dac.set_output(&cx.shared.outputs);
        cx.local.dac.update();
        dac_update::spawn_after(10.millis()).unwrap();
    }

    #[task(shared = [desired_out])]
    fn button(mut cx: button::Context, event: ButtonEvent) {
        let mut buf = ArrayString::new();

        write!(&mut buf, "Btn {:?}\n", event).expect("!write");
        print::spawn(buf, Point::new(5, 35)).ok();
        match event {
            ButtonEvent {
                button: Button::TopLeft,
                down: true,
            } => {
                if cx.shared.desired_out.dac0 > 1f32 {
                    cx.shared.desired_out.dac0 -= 0.5
                }
            }
            ButtonEvent {
                button: Button::TopMiddle,
                down: true,
            } => {

                if cx.shared.desired_out.dac0 < 20f32 {
                    cx.shared.desired_out.dac0 += 0.5
                }
            }
            ButtonEvent {
                button: Button::Down,
                down: true,
            } => {
                if cx.shared.desired_out.dac1 > 1f32 {
                    cx.shared.desired_out.dac1 -= 0.5
                }
            }
            ButtonEvent {
                button: Button::Up,
                down: true,
            } => {

                if cx.shared.desired_out.dac1 < 20f32 {
                    cx.shared.desired_out.dac1 += 0.5
                }
            }
            ButtonEvent { .. } => {}
        }
    }

    #[task(binds = ADC0_RESRDY, local = [i_adc0], shared = [inputs])]
    fn adc0_rdy(mut cx: adc0_rdy::Context) {
        let Some(sample) = cx.local.i_adc0.service_interrupt_ready() else {
            return;
        };
        cx.shared.inputs.raw_adc_a0 = sample;
    }

    #[task(binds = ADC1_RESRDY, local = [i_adc1], shared = [inputs])]
    fn adc1_rdy(mut cx: adc1_rdy::Context) {
        let Some(sample) = cx.local.i_adc1.service_interrupt_ready() else {
            return;
        };
        cx.shared.inputs.raw_adc_a1 = sample;
    }

    #[task(shared = [inputs, outputs, state])]
    fn print_state(cx: print_state::Context) {
        fn fmt(num: usize, side: &crate::logics::Side, adc: u16, dac: u16, txt: &str) -> ArrayString<[u8; 256]> {
            let mut buf = ArrayString::new();
            fn to_voltage(raw: u16) -> f32 {
                (raw as f32) / 4096.0 * 3300f32
            }
            write!(&mut buf,
r"ADC{}:
 Raw:
   {:>9}
   {:>7.2}mV
 Converted:
   {:>08.4}V
{}:
  Desired Output:
    {:>05.1}V
  Raw:
    {:>9}
    {:>7.2}mV
  Real:
    {:>05.1}V
",
                   num,
                   adc,
                   to_voltage(adc),
                   side.input,
                   txt,
                   side.desired_output,
                   dac,
                   to_voltage(dac),
                   side.real_output
            ).expect("!write");
            buf
        }
        {
            // let sample = 1;
            // const MAX_VOLTAGE_RATIO: f32 = 3f32 / 3.3;
            // let percent = sample as f32 / 4096f32;
            // let volt = percent * 3300f32;
            // let real = (percent / MAX_VOLTAGE_RATIO) * 20000f32;
            // let mut buf = ArrayString::new();
            // write!(&mut buf, "A1 Raw:{:>9.2}mV\n   Real:{:>8.2}mV", volt, real).expect("!write");
            let buf = fmt(0, &cx.shared.state.left, cx.shared.inputs.raw_adc_a0, cx.shared.outputs.dac1, "Left to Right");
            print::spawn(buf, Point::new(5, 30)).ok();
            let buf = fmt(1, &cx.shared.state.right, cx.shared.inputs.raw_adc_a1, cx.shared.outputs.dac0, "Right to Left");
            print::spawn(buf, Point::new(160, 30)).ok();
        }
        print_state::spawn_after(200.millis()).unwrap();
    }


    // task from macro does not currently work using pre-generated
    // ```
    // use crate::buttons::prelude::*;
    // button_interrupt!(button_ctr, button);
    // ```
    #[task(binds = EIC_EXTINT_3, shared = [ button_ctr ])]
    fn _btn_intr_3(mut cx: _btn_intr_3::Context) {
        if let Some(event) = cx.shared.button_ctr.interrupt_extint3() {
            button::spawn(event).ok();
        }
    }

    #[task(binds = EIC_EXTINT_4, shared = [ button_ctr ])]
    fn _btn_intr_4(mut cx: _btn_intr_4::Context) {
        if let Some(event) = cx.shared.button_ctr.interrupt_extint4() {
            button::spawn(event).ok();
        }
    }

    #[task(binds = EIC_EXTINT_5, shared = [ button_ctr ])]
    fn _btn_intr_5(mut cx: _btn_intr_5::Context) {
        if let Some(event) = cx.shared.button_ctr.interrupt_extint5() {
            button::spawn(event).ok();
        }
    }

    #[task(binds = EIC_EXTINT_7, shared = [ button_ctr ])]
    fn _btn_intr_7(mut cx: _btn_intr_7::Context) {
        if let Some(event) = cx.shared.button_ctr.interrupt_extint7() {
            button::spawn(event).ok();
        }
    }

    #[task(binds = EIC_EXTINT_10, shared = [ button_ctr ])]
    fn _btn_intr_10(mut cx: _btn_intr_10::Context) {
        if let Some(event) = cx.shared.button_ctr.interrupt_extint10() {
            button::spawn(event).ok();
        }
    }

    #[task(binds = EIC_EXTINT_11, shared = [ button_ctr ])]
    fn _btn_intr_11(mut cx: _btn_intr_11::Context) {
        if let Some(event) = cx.shared.button_ctr.interrupt_extint11() {
            button::spawn(event).ok();
        }
    }

    #[task(binds = EIC_EXTINT_12, shared = [ button_ctr ])]
    fn _btn_intr_12(mut cx: _btn_intr_12::Context) {
        if let Some(event) = cx.shared.button_ctr.interrupt_extint12() {
            button::spawn(event).ok();
        }
    }
}
