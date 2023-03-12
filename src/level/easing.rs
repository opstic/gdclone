use std::f64::consts::PI;

const PI_2: f64 = PI * 2.;

#[derive(Copy, Clone, Default, PartialEq)]
pub(crate) enum Easing {
    #[default]
    None,

    EaseInOut(f64),
    EaseIn(f64),
    EaseOut(f64),

    ElasticInOut(f64),
    ElasticIn(f64),
    ElasticOut(f64),

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
    pub(crate) fn from_id(id: u8, rate: Option<f64>) -> Easing {
        match id {
            0 => Easing::None,
            1 => Easing::EaseInOut(rate.unwrap()),
            2 => Easing::EaseIn(rate.unwrap()),
            3 => Easing::EaseOut(rate.unwrap()),
            4 => Easing::ElasticInOut(rate.unwrap()),
            5 => Easing::ElasticIn(rate.unwrap()),
            6 => Easing::ElasticOut(rate.unwrap()),
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

    pub(crate) fn sample(self, x: f64) -> f64 {
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

    fn ease_in_out(x: f64, rate: f64) -> f64 {
        let x = x * 2.;
        if x < 1. {
            0.5 * f64::powf(x, rate)
        } else {
            1. - 0.5 * f64::powf(2. - x, rate)
        }
    }

    fn ease_in(x: f64, rate: f64) -> f64 {
        f64::powf(x, rate)
    }

    fn ease_out(x: f64, rate: f64) -> f64 {
        f64::powf(x, 1. / rate)
    }

    fn elastic_in_out(x: f64, period: f64) -> f64 {
        if x == 0. || x == 1. {
            x
        } else {
            let mut period = period;
            if period == 0. {
                period = 0.3 * 1.5;
            }
            let s = period / 4.;
            let x = x - 1.;
            if x < 0. {
                -0.5 * f64::powf(2., 10. * x) * f64::sin((x - s) * PI_2 / period)
            } else {
                f64::powf(2., -10. * x) * f64::sin((x - s) * PI_2 / period) * 0.5 + 1.
            }
        }
    }

    fn elastic_in(x: f64, period: f64) -> f64 {
        if x == 0. || x == 1. {
            x
        } else {
            let s = period / 4.;
            let x = x - 1.;
            -f64::powf(2., 10. * x) * f64::sin((x - s) * PI_2 / period)
        }
    }

    fn elastic_out(x: f64, period: f64) -> f64 {
        if x == 0. || x == 1. {
            x
        } else {
            let s = period / 4.;
            f64::powf(2., -10. * x) * f64::sin((x - s) * PI_2 / period) + 1.
        }
    }

    fn bounce_time(x: f64) -> f64 {
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

    fn bounce_in_out(x: f64) -> f64 {
        if x < 0.5 {
            (1. - Self::bounce_time(1. - x * 2.)) * 0.5
        } else {
            Self::bounce_time(x * 2. - 1.) * 0.5 + 0.5
        }
    }

    fn bounce_in(x: f64) -> f64 {
        1. - Self::bounce_time(1. - x)
    }

    fn bounce_out(x: f64) -> f64 {
        Self::bounce_time(x)
    }

    fn exponential_in_out(x: f64) -> f64 {
        if x == 0. || x == 1. {
            x
        } else if x < 0.5 {
            0.5 * f64::powf(2., 10. * (x * 2. - 1.))
        } else {
            0.5 * (-f64::powf(2., -10. * (x * 2. - 1.)) + 2.)
        }
    }

    fn exponential_in(x: f64) -> f64 {
        if x == 0. {
            x
        } else {
            f64::powf(2., 10. * (x / 1. - 1.)) - 1. * 0.001
        }
    }

    fn exponential_out(x: f64) -> f64 {
        if x == 1. {
            x
        } else {
            -f64::powf(2., -10. * x / 1.) + 1.
        }
    }

    // cocos sine easings weren't working, so i just took the version https://easings.net provided
    fn sine_in_out(x: f64) -> f64 {
        -0.5 * (f64::cos(x * PI) - 1.)
    }

    fn sine_in(x: f64) -> f64 {
        1. - f64::cos((x * PI) / 2.)
    }

    fn sine_out(x: f64) -> f64 {
        f64::sin((x * PI) / 2.)
    }

    fn back_in_out(x: f64) -> f64 {
        let overshoot = 1.70158 * 1.525;
        let x = x * 2.;
        if x < 1. {
            (x * x * ((overshoot + 1.) * x - overshoot)) / 2.
        } else {
            let x = x - 2.;
            (x * x * ((overshoot + 1.) * x + overshoot)) / 2. + 1.
        }
    }

    fn back_in(x: f64) -> f64 {
        let overshoot = 1.70158;
        x * x * ((overshoot + 1.) * x - overshoot)
    }

    fn back_out(x: f64) -> f64 {
        let overshoot = 1.70158;
        let x = x - 1.;
        x * x * ((overshoot + 1.) * x + overshoot) + 1.
    }
}
