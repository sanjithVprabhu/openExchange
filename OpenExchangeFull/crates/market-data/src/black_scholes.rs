use std::f64::consts::PI;
use crate::types::{BSInputs, Greeks, OptionType};

pub const MIN_TIME: f64 = 1.0 / (365.25 * 24.0 * 3600.0);
pub const MIN_VOL: f64 = 0.01;
pub const MAX_VOL: f64 = 5.0;
pub const MIN_PRICE: f64 = 1e-6;

pub fn norm_pdf(x: f64) -> f64 {
    (1.0 / (2.0 * PI).sqrt()) * (-0.5 * x * x).exp()
}

pub fn norm_cdf(x: f64) -> f64 {
    let k = 1.0 / (1.0 + 0.2316419 * x.abs());
    let poly = k * (0.319381530
        + k * (-0.356563782
        + k * (1.781477937
        + k * (-1.821255978
        + k * 1.330274429))));
    
    let approx = 1.0 - norm_pdf(x) * poly;
    
    if x >= 0.0 {
        approx
    } else {
        1.0 - approx
    }
}

pub fn d1_d2(input: &BSInputs) -> (f64, f64) {
    let s = input.spot;
    let k = input.strike;
    let t = input.time.max(1e-6);
    let v = input.vol.max(1e-6);
    let r = input.rate;
    
    let d1 = ((s / k).ln() + (r + 0.5 * v * v) * t) / (v * t.sqrt());
    let d2 = d1 - v * t.sqrt();
    
    (d1, d2)
}

pub fn black_scholes_price(mut input: BSInputs) -> f64 {
    input.validate();
    
    let (d1, d2) = d1_d2(&input);
    let s = input.spot;
    let k = input.strike;
    let t = input.time;
    let r = input.rate;
    
    let price = match input.option_type {
        OptionType::Call => {
            s * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
        }
        OptionType::Put => {
            k * (-r * t).exp() * norm_cdf(-d2) - s * norm_cdf(-d1)
        }
    };
    
    price.max(0.0)
}

pub fn intrinsic_value(spot: f64, strike: f64, option_type: OptionType) -> f64 {
    match option_type {
        OptionType::Call => (spot - strike).max(0.0),
        OptionType::Put => (strike - spot).max(0.0),
    }
}

pub fn black_scholes_greeks(mut input: BSInputs) -> Greeks {
    input.validate();
    
    let (d1, d2) = d1_d2(&input);
    let s = input.spot;
    let k = input.strike;
    let t = input.time;
    let v = input.vol;
    let r = input.rate;
    
    let pdf = norm_pdf(d1);
    let sqrt_t = t.sqrt();
    
    let delta = match input.option_type {
        OptionType::Call => norm_cdf(d1),
        OptionType::Put => norm_cdf(d1) - 1.0,
    };
    
    let gamma = pdf / (s * v * sqrt_t);
    
    let vega = s * pdf * sqrt_t;
    
    let theta = match input.option_type {
        OptionType::Call => {
            -(s * pdf * v) / (2.0 * sqrt_t) - r * k * (-r * t).exp() * norm_cdf(d2)
        }
        OptionType::Put => {
            -(s * pdf * v) / (2.0 * sqrt_t) + r * k * (-r * t).exp() * norm_cdf(-d2)
        }
    };
    
    let rho = match input.option_type {
        OptionType::Call => k * t * (-r * t).exp() * norm_cdf(d2),
        OptionType::Put => -k * t * (-r * t).exp() * norm_cdf(-d2),
    };
    
    Greeks {
        delta,
        gamma,
        vega,
        theta,
        rho,
    }
}

pub fn implied_volatility(
    market_price: f64,
    mut input: BSInputs,
) -> Option<f64> {
    let mut vol = 0.3;
    
    for _ in 0..100 {
        input.vol = vol;
        
        let price = black_scholes_price(input);
        let vega = black_scholes_greeks(input).vega;
        
        if (price - market_price).abs() < 1e-6 {
            return Some(vol);
        }
        
        if vega.abs() < 1e-8 {
            break;
        }
        
        vol -= (price - market_price) / vega;
        vol = vol.clamp(MIN_VOL, MAX_VOL);
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_call_price_itm() {
        let input = BSInputs {
            spot: 60000.0,
            strike: 50000.0,
            time: 30.0 / 365.25,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Call,
        };
        
        let price = black_scholes_price(input);
        assert!(price >= 10000.0);
    }
    
    #[test]
    fn test_put_price_otm() {
        let input = BSInputs {
            spot: 60000.0,
            strike: 50000.0,
            time: 30.0 / 365.25,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Put,
        };
        
        let price = black_scholes_price(input);
        assert!(price > 0.0 && price < 1000.0);
    }
    
    #[test]
    fn test_call_delta_positive() {
        let input = BSInputs {
            spot: 50000.0,
            strike: 50000.0,
            time: 30.0 / 365.25,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Call,
        };
        
        let greeks = black_scholes_greeks(input);
        assert!(greeks.delta > 0.4 && greeks.delta < 0.6);
    }
    
    #[test]
    fn test_put_call_parity() {
        let spot = 50000.0;
        let strike = 50000.0;
        let time = 30.0 / 365.25;
        let vol = 0.5;
        let rate = 0.0;
        
        let call = black_scholes_price(BSInputs {
            spot, strike, time, vol, rate,
            option_type: OptionType::Call,
        });
        
        let put = black_scholes_price(BSInputs {
            spot, strike, time, vol, rate,
            option_type: OptionType::Put,
        });
        
        let parity_lhs = call - put;
        let parity_rhs = spot - strike * (-rate * time).exp();
        
        assert!((parity_lhs - parity_rhs).abs() < 1.0);
    }
    
    #[test]
    fn test_implied_vol_roundtrip() {
        let input = BSInputs {
            spot: 50000.0,
            strike: 50000.0,
            time: 30.0 / 365.25,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Call,
        };
        
        let price = black_scholes_price(input);
        let recovered_vol = implied_volatility(price, input).unwrap();
        
        assert!((recovered_vol - 0.5).abs() < 0.01);
    }
    
    #[test]
    fn test_intrinsic_value_call() {
        let intrinsic = intrinsic_value(60000.0, 50000.0, OptionType::Call);
        assert!((intrinsic - 10000.0).abs() < 0.01);
    }
    
    #[test]
    fn test_intrinsic_value_put() {
        let intrinsic = intrinsic_value(40000.0, 50000.0, OptionType::Put);
        assert!((intrinsic - 10000.0).abs() < 0.01);
    }
    
    #[test]
    fn test_otm_call_intrinsic_zero() {
        let intrinsic = intrinsic_value(40000.0, 50000.0, OptionType::Call);
        assert!(intrinsic.abs() < 0.01);
    }
    
    #[test]
    fn test_otm_put_intrinsic_zero() {
        let intrinsic = intrinsic_value(60000.0, 50000.0, OptionType::Put);
        assert!(intrinsic.abs() < 0.01);
    }
    
    #[test]
    fn test_norm_cdf_symmetry() {
        assert!((norm_cdf(0.5) + norm_cdf(-0.5) - 1.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_norm_cdf_extreme() {
        assert!((norm_cdf(10.0) - 1.0).abs() < 1e-10);
        assert!(norm_cdf(-10.0).abs() < 1e-10);
    }
    
    #[test]
    fn test_near_expiry() {
        let input = BSInputs {
            spot: 60000.0,
            strike: 50000.0,
            time: 0.0001,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Call,
        };
        
        let price = black_scholes_price(input);
        let intrinsic = intrinsic_value(60000.0, 50000.0, OptionType::Call);
        
        assert!((price - intrinsic).abs() < 100.0);
    }
    
    #[test]
    fn test_deep_itm_call() {
        let input = BSInputs {
            spot: 100000.0,
            strike: 10000.0,
            time: 30.0 / 365.25,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Call,
        };
        
        let price = black_scholes_price(input);
        let intrinsic = intrinsic_value(100000.0, 10000.0, OptionType::Call);
        
        assert!(price > intrinsic * 0.99);
    }
    
    #[test]
    fn test_gamma_positive() {
        let input = BSInputs {
            spot: 50000.0,
            strike: 50000.0,
            time: 30.0 / 365.25,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Call,
        };
        
        let greeks = black_scholes_greeks(input);
        assert!(greeks.gamma > 0.0);
    }
    
    #[test]
    fn test_vega_positive() {
        let input = BSInputs {
            spot: 50000.0,
            strike: 50000.0,
            time: 30.0 / 365.25,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Call,
        };
        
        let greeks = black_scholes_greeks(input);
        assert!(greeks.vega > 0.0);
    }
    
    #[test]
    fn test_theta_negative_long_call() {
        let input = BSInputs {
            spot: 50000.0,
            strike: 50000.0,
            time: 30.0 / 365.25,
            vol: 0.5,
            rate: 0.0,
            option_type: OptionType::Call,
        };
        
        let greeks = black_scholes_greeks(input);
        assert!(greeks.theta < 0.0);
    }
}
