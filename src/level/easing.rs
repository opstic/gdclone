use std::f32::consts::{PI, TAU};

#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) enum Easing {
    #[default]
    None,

    EaseInOut(f32),
    EaseIn(f32),
    EaseOut(f32),

    ElasticInOut(f32),
    ElasticIn(f32),
    ElasticOut(f32),

    BounceInOut,
    BounceIn,
    BounceOut,

    ExponentialInOut,
    ExponentialIn,
    ExponentialOut,

    SineInOut,
    SineIn,
    SineOut,

    BackInOut,
    BackIn,
    BackOut,
}

impl Easing {
    pub(crate) fn from_id(id: u8, rate: Option<f32>) -> Easing {
        match id {
            0 => Easing::None,
            1 => Easing::EaseInOut(rate.unwrap_or(2.)),
            2 => Easing::EaseIn(rate.unwrap_or(2.)),
            3 => Easing::EaseOut(rate.unwrap_or(2.)),
            4 => Easing::ElasticInOut(rate.unwrap_or(2.)),
            5 => Easing::ElasticIn(rate.unwrap_or(2.)),
            6 => Easing::ElasticOut(rate.unwrap_or(2.)),
            7 => Easing::BounceInOut,
            8 => Easing::BounceIn,
            9 => Easing::BounceOut,
            10 => Easing::ExponentialInOut,
            11 => Easing::ExponentialIn,
            12 => Easing::ExponentialOut,
            13 => Easing::SineInOut,
            14 => Easing::SineIn,
            15 => Easing::SineOut,
            16 => Easing::BackInOut,
            17 => Easing::BackIn,
            18 => Easing::BackOut,
            _ => Easing::None,
        }
    }

    pub(crate) fn sample(self, x: f32) -> f32 {
        if x == 0. || x == 1. {
            return x;
        }
        match self {
            Easing::None => x,
            Easing::EaseInOut(rate) => Self::ease_in_out(x, rate),
            Easing::EaseIn(rate) => Self::ease_in(x, rate),
            Easing::EaseOut(rate) => Self::ease_out(x, rate),
            Easing::ElasticInOut(period) => Self::elastic_in_out(x, period),
            Easing::ElasticIn(period) => Self::elastic_in(x, period),
            Easing::ElasticOut(period) => Self::elastic_out(x, period),
            Easing::BounceInOut => Self::bounce_in_out(x),
            Easing::BounceIn => Self::bounce_in(x),
            Easing::BounceOut => Self::bounce_out(x),
            Easing::ExponentialInOut => Self::exponential_in_out(x),
            Easing::ExponentialIn => Self::exponential_in(x),
            Easing::ExponentialOut => Self::exponential_out(x),
            Easing::SineInOut => Self::sine_in_out(x),
            Easing::SineIn => Self::sine_in(x),
            Easing::SineOut => Self::sine_out(x),
            Easing::BackInOut => Self::back_in_out(x),
            Easing::BackIn => Self::back_in(x),
            Easing::BackOut => Self::back_out(x),
        }
    }

    fn ease_in_out(x: f32, rate: f32) -> f32 {
        let x = x * 2.;
        if x < 1. {
            0.5 * f32::powf(x, rate)
        } else {
            1. - 0.5 * f32::powf(2. - x, rate)
        }
    }

    fn ease_in(x: f32, rate: f32) -> f32 {
        f32::powf(x, rate)
    }

    fn ease_out(x: f32, rate: f32) -> f32 {
        f32::powf(x, 1. / rate)
    }

    fn elastic_in_out(x: f32, period: f32) -> f32 {
        let mut period = period;
        if period == 0. {
            period = 0.3 * 1.5;
        }
        let s = period / 4.;
        let x = x - 1.;
        if x < 0. {
            -0.5 * f32::powf(2., 10. * x) * f32::sin((x - s) * TAU / period)
        } else {
            f32::powf(2., -10. * x) * f32::sin((x - s) * TAU / period) * 0.5 + 1.
        }
    }

    fn elastic_in(x: f32, period: f32) -> f32 {
        let s = period / 4.;
        let x = x - 1.;
        -f32::powf(2., 10. * x) * f32::sin((x - s) * TAU / period)
    }

    fn elastic_out(x: f32, period: f32) -> f32 {
        let s = period / 4.;
        f32::powf(2., -10. * x) * f32::sin((x - s) * TAU / period) + 1.
    }

    fn bounce_time(x: f32) -> f32 {
        if x < 1. / 2.75 {
            7.5625 * x * x
        } else if x < 2. / 2.75 {
            let x = x - 1.5 / 2.75;
            7.5625 * x * x + 0.75
        } else if x < 2.5 / 2.75 {
            let x = x - 2.25 / 2.75;
            7.5625 * x * x + 0.9375
        } else {
            let x = x - 2.625 / 2.75;
            7.5625 * x * x + 0.984375
        }
    }

    fn bounce_in_out(x: f32) -> f32 {
        if x < 0.5 {
            (1. - Self::bounce_time(1. - x * 2.)) * 0.5
        } else {
            Self::bounce_time(x * 2. - 1.) * 0.5 + 0.5
        }
    }

    fn bounce_in(x: f32) -> f32 {
        1. - Self::bounce_time(1. - x)
    }

    fn bounce_out(x: f32) -> f32 {
        Self::bounce_time(x)
    }

    fn exponential_in_out(x: f32) -> f32 {
        if x < 0.5 {
            0.5 * f32::powf(2., 10. * (x * 2. - 1.))
        } else {
            0.5 * (-f32::powf(2., -10. * (x * 2. - 1.)) + 2.)
        }
    }

    fn exponential_in(x: f32) -> f32 {
        f32::powf(2., 10. * (x - 1.)) - 1. * 0.001
    }

    fn exponential_out(x: f32) -> f32 {
        -f32::powf(2., -10. * x) + 1.
    }

    // cocos sine easings weren't working, so i just took the version https://easings.net provided
    fn sine_in_out(x: f32) -> f32 {
        -0.5 * (f32::cos(x * PI) - 1.)
    }

    fn sine_in(x: f32) -> f32 {
        1. - f32::cos((x * PI) / 2.)
    }

    fn sine_out(x: f32) -> f32 {
        f32::sin((x * PI) / 2.)
    }

    fn back_in_out(x: f32) -> f32 {
        let overshoot = 1.70158 * 1.525;
        let x = x * 2.;
        if x < 1. {
            (x * x * ((overshoot + 1.) * x - overshoot)) / 2.
        } else {
            let x = x - 2.;
            (x * x * ((overshoot + 1.) * x + overshoot)) / 2. + 1.
        }
    }

    fn back_in(x: f32) -> f32 {
        let overshoot = 1.70158;
        x * x * ((overshoot + 1.) * x - overshoot)
    }

    fn back_out(x: f32) -> f32 {
        let overshoot = 1.70158;
        let x = x - 1.;
        x * x * ((overshoot + 1.) * x + overshoot) + 1.
    }
}
