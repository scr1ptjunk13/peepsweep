use rust_decimal::Decimal;
use rust_decimal::prelude::*;

/// Mathematical utility functions for Decimal types
pub trait DecimalMath {
    fn powd(self, exponent: Decimal) -> Option<Decimal>;
    fn ln(self) -> Option<Decimal>;
    fn sqrt(self) -> Option<Decimal>;
}

impl DecimalMath for Decimal {
    /// Power function for Decimal
    fn powd(self, exponent: Decimal) -> Option<Decimal> {
        if self <= Decimal::ZERO {
            return None;
        }
        
        let base_f64 = self.to_f64()?;
        let exp_f64 = exponent.to_f64()?;
        
        let result = libm::pow(base_f64, exp_f64);
        
        if result.is_finite() {
            Decimal::from_f64(result)
        } else {
            None
        }
    }
    
    /// Natural logarithm for Decimal
    fn ln(self) -> Option<Decimal> {
        if self <= Decimal::ZERO {
            return None;
        }
        
        let f64_val = self.to_f64()?;
        let result = libm::log(f64_val);
        
        if result.is_finite() {
            Decimal::from_f64(result)
        } else {
            None
        }
    }
    
    /// Square root for Decimal
    fn sqrt(self) -> Option<Decimal> {
        if self < Decimal::ZERO {
            return None;
        }
        
        if self == Decimal::ZERO {
            return Some(Decimal::ZERO);
        }
        
        let f64_val = self.to_f64()?;
        let result = libm::sqrt(f64_val);
        
        if result.is_finite() {
            Decimal::from_f64(result)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_sqrt() {
        assert_eq!(dec!(4).sqrt(), Some(dec!(2)));
        assert_eq!(dec!(9).sqrt(), Some(dec!(3)));
        assert_eq!(dec!(0).sqrt(), Some(dec!(0)));
        assert_eq!(dec!(-1).sqrt(), None);
    }

    #[test]
    fn test_ln() {
        let e = Decimal::from_f64(std::f64::consts::E).unwrap();
        assert!((e.ln().unwrap() - dec!(1)).abs() < dec!(0.001));
        assert_eq!(dec!(0).ln(), None);
        assert_eq!(dec!(-1).ln(), None);
    }

    #[test]
    fn test_powd() {
        assert_eq!(dec!(2).powd(dec!(3)), Some(dec!(8)));
        assert_eq!(dec!(0).powd(dec!(2)), None);
        assert_eq!(dec!(-1).powd(dec!(2)), None);
    }
}
