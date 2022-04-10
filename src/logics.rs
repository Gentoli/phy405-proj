use crate::state::{InputValues, OutputValues};
use micromath::F32Ext;

#[derive(Debug, Clone, Default)]
pub struct State {
    pub left: Side,
    pub right: Side,
}

#[derive(Debug, Clone, Default)]
pub struct Side {
    pub input: f32,
    pub desired_output: f32,
    pub real_output: f32,
}

#[derive(Default)]
pub struct DesiredOutput {
    pub dac0: f32,
    pub dac1: f32,
}

impl State {
    pub fn from(input: &InputValues, desired_out: &DesiredOutput) -> Self {
        let mut s = Self {
            left: Side {
                input: Self::adc_convert(input.raw_adc_a0).1,
                desired_output: desired_out.dac0,
                real_output: 0.0,
            },
            right: Side {
                input: Self::adc_convert(input.raw_adc_a1).1,
                desired_output: desired_out.dac1,
                real_output: 0.0,
            },
        };

        s.right.real_output = s.left.output_for(&s.right).0;
        s.left.real_output = s.right.output_for(&s.left).0;
        s
    }

    pub fn get_output_level(&self) -> OutputValues {
        OutputValues{
            dac0: Self::dac_convert(self.right.real_output),
            dac1: Self::dac_convert(self.left.real_output),
        }
    }

    fn adc_convert(raw: u16) -> (f32, f32) {
        const MAX_VOLTAGE_RATIO: f32 = 3f32/3.3;
        let percent = raw as f32 / 4096f32;
        let volt = percent * 3.3;
        let real = (percent / MAX_VOLTAGE_RATIO) * 20.0;
        (volt, real)
    }
    fn dac_convert(out_voltage: f32) -> u16 {
        ((out_voltage / 20.0) * 4096.0) as u16
    }
}

impl Side {
    fn output_for(&self, want: &Side) -> (f32, bool) {
        if self.input < 1.0 {
            (want.desired_output, true)
        }
        else if self.input < want.desired_output {
            (self.input, true)
        }
        else {
            (0.0, false)
        }
    }
}