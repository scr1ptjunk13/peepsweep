use rust_decimal::Decimal;
use rust_decimal::prelude::*;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

use crate::{CalculationResult, CalculationError};
use crate::database::models::{UserPositionSummary, PositionV2, PositionV3};
use crate::utils::DecimalMath;

/// Fee calculation results
#[derive(Debug, Clone)]
pub struct FeeResult {
    pub fees_earned_token0: Decimal,
    pub fees_earned_token1: Decimal,
    pub fees_earned_usd: Decimal,
    pub fee_apr: Option<Decimal>,
    pub daily_fees_usd: Option<Decimal>,
    pub projected_monthly_fees: Option<Decimal>,
    pub projected_yearly_fees: Option<Decimal>,
}

/// Fee tier information for Uniswap V3
#[derive(Debug, Clone)]
pub struct FeeTier {
    pub fee_rate: Decimal,
    pub tick_spacing: i32,
}

/// Historical fee data point
#[derive(Debug, Clone)]
pub struct FeeDataPoint {
    pub timestamp: DateTime<Utc>,
    pub fees_token0: Decimal,
    pub fees_token1: Decimal,
    pub fees_usd: Decimal,
    pub volume_usd: Decimal,
}

#[derive(Debug)]
pub struct FeesCalculator {
    fee_tiers: HashMap<u32, FeeTier>,
}

impl Default for FeesCalculator {
    fn default() -> Self {
        let mut fee_tiers = HashMap::new();
        
        // Uniswap V3 fee tiers
        fee_tiers.insert(100, FeeTier { fee_rate: Decimal::from_str("0.0001").unwrap(), tick_spacing: 1 });
        fee_tiers.insert(500, FeeTier { fee_rate: Decimal::from_str("0.0005").unwrap(), tick_spacing: 10 });
        fee_tiers.insert(3000, FeeTier { fee_rate: Decimal::from_str("0.003").unwrap(), tick_spacing: 60 });
        fee_tiers.insert(10000, FeeTier { fee_rate: Decimal::from_str("0.01").unwrap(), tick_spacing: 200 });

        Self { fee_tiers }
    }
}

impl FeesCalculator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate fees earned for a Uniswap V2 position
    pub fn calculate_v2_fees(
        &self,
        position: &PositionV2,
        current_reserve0: Decimal,
        current_reserve1: Decimal,
        total_supply: Decimal,
    ) -> CalculationResult<FeeResult> {
        if total_supply.is_zero() {
            return Err(CalculationError::InvalidInput("Total supply cannot be zero".to_string()));
        }

        // V2 fees are proportional to liquidity share
        let liquidity_share = position.liquidity / total_supply;
        
        // Estimate fees based on position's share of total reserves
        let estimated_fees_token0 = Decimal::ZERO; // Not available in V2 model
        let estimated_fees_token1 = Decimal::ZERO; // Not available in V2 model

        let fees_usd = position.fees_earned_usd.unwrap_or(Decimal::ZERO);

        let fee_apr = self.calculate_fee_apr(
            fees_usd,
            Decimal::ZERO, // position value placeholder
            position.days_active().unwrap_or(1) as i32,
        )?;

        Ok(FeeResult {
            fees_earned_token0: estimated_fees_token0,
            fees_earned_token1: estimated_fees_token1,
            fees_earned_usd: fees_usd,
            fee_apr: Some(fee_apr),
            daily_fees_usd: if position.days_active().unwrap_or(0) > 0 {
                Some(fees_usd / Decimal::from(position.days_active().unwrap() as i32))
            } else {
                None
            },
            projected_monthly_fees: Some(fees_usd / Decimal::from(position.days_active().unwrap_or(1) as i32) * Decimal::from(30)),
            projected_yearly_fees: Some(fee_apr * Decimal::ZERO / Decimal::from(100)), // position value not available
        })
    }

    /// Calculate fees earned for a Uniswap V3 position
    pub fn calculate_v3_fees(
        &self,
        position: &PositionV3,
        fee_growth_global_0: Decimal,
        fee_growth_global_1: Decimal,
        fee_growth_inside_0: Decimal,
        fee_growth_inside_1: Decimal,
    ) -> CalculationResult<FeeResult> {
        // Calculate uncollected fees based on fee growth
        let uncollected_fees_0 = fee_growth_inside_0; // fee_growth_inside_0_last not available in V3 model
        let uncollected_fees_1 = fee_growth_inside_1; // fee_growth_inside_1_last not available in V3 model

        // Total fees = collected + uncollected
        let total_fees_token0 = uncollected_fees_0; // No stored fees in V3 model
        let total_fees_token1 = uncollected_fees_1; // No stored fees in V3 model

        let fees_usd = position.fees_earned_usd.unwrap_or(Decimal::ZERO);

        let fee_apr = self.calculate_fee_apr(
            fees_usd,
            Decimal::ZERO, // position value placeholder
            position.days_active().unwrap_or(1) as i32,
        )?;

        Ok(FeeResult {
            fees_earned_token0: total_fees_token0,
            fees_earned_token1: total_fees_token1,
            fees_earned_usd: fees_usd,
            fee_apr: Some(fee_apr),
            daily_fees_usd: if position.days_active().unwrap_or(0) > 0 {
                Some(fees_usd / Decimal::from(position.days_active().unwrap() as i32))
            } else {
                None
            },
            projected_monthly_fees: Some(fees_usd / Decimal::from(position.days_active().unwrap_or(1) as i32) * Decimal::from(30)),
            projected_yearly_fees: Some(fee_apr * Decimal::ZERO / Decimal::from(100)), // position value not available
        })
    }

    /// Calculate fee APR
    pub fn calculate_fee_apr(
        &self,
        fees_earned_usd: Decimal,
        position_value_usd: Decimal,
        days_elapsed: i32,
    ) -> CalculationResult<Decimal> {
        if position_value_usd.is_zero() || days_elapsed <= 0 {
            return Ok(Decimal::ZERO);
        }

        let daily_return = fees_earned_usd / position_value_usd / Decimal::from(days_elapsed);
        Ok(daily_return * Decimal::from(365) * Decimal::from(100))
    }

    /// Calculate projected fees based on historical data
    pub fn calculate_projected_fees(
        &self,
        historical_data: &[FeeDataPoint],
        position_value_usd: Decimal,
        projection_days: i64,
    ) -> CalculationResult<Decimal> {
        if historical_data.is_empty() || position_value_usd.is_zero() {
            return Ok(Decimal::ZERO);
        }

        // Calculate average daily fee rate from historical data
        let mut total_daily_rate = Decimal::ZERO;
        let mut valid_days = 0;

        for window in historical_data.windows(2) {
            if let [prev, curr] = window {
                let days_diff = (curr.timestamp - prev.timestamp).num_days();
                if days_diff > 0 {
                    let daily_fees = (curr.fees_usd - prev.fees_usd) / Decimal::from(days_diff);
                    let daily_rate = daily_fees / position_value_usd;
                    total_daily_rate += daily_rate;
                    valid_days += 1;
                }
            }
        }

        if valid_days == 0 {
            return Ok(Decimal::ZERO);
        }

        let avg_daily_rate = total_daily_rate / Decimal::from(valid_days);
        Ok(avg_daily_rate * position_value_usd * Decimal::from(projection_days))
    }

    /// Calculate fee efficiency (fees per dollar of liquidity)
    pub fn calculate_fee_efficiency(
        &self,
        fees_earned_usd: Decimal,
        average_liquidity_usd: Decimal,
        days_active: i32,
    ) -> CalculationResult<Decimal> {
        if average_liquidity_usd.is_zero() || days_active <= 0 {
            return Ok(Decimal::ZERO);
        }

        let daily_fees = fees_earned_usd / Decimal::from(days_active);
        Ok(daily_fees / average_liquidity_usd)
    }

    /// Calculate optimal fee tier for a V3 position based on volatility
    pub fn suggest_optimal_fee_tier(
        &self,
        token0_volatility: Decimal,
        token1_volatility: Decimal,
        expected_volume_usd: Decimal,
    ) -> u32 {
        let avg_volatility = (token0_volatility + token1_volatility) / Decimal::from(2);

        // High volatility pairs should use higher fee tiers
        if avg_volatility > Decimal::from_str("0.5").unwrap() {
            10000 // 1%
        } else if avg_volatility > Decimal::from_str("0.2").unwrap() {
            3000 // 0.3%
        } else if avg_volatility > Decimal::from_str("0.05").unwrap() {
            500 // 0.05%
        } else {
            100 // 0.01%
        }
    }

    /// Calculate impermanent loss adjusted fee returns
    pub fn calculate_il_adjusted_returns(
        &self,
        fees_earned_usd: Decimal,
        impermanent_loss_usd: Decimal,
    ) -> Decimal {
        fees_earned_usd - impermanent_loss_usd
    }

    /// Get fee tier information
    pub fn get_fee_tier(&self, fee_tier: u32) -> Option<&FeeTier> {
        self.fee_tiers.get(&fee_tier)
    }

    /// Calculate breakeven time (days needed for fees to offset IL)
    pub fn calculate_breakeven_time(
        &self,
        current_il_usd: Decimal,
        daily_fees_usd: Decimal,
    ) -> Option<i32> {
        if daily_fees_usd.is_zero() || current_il_usd <= Decimal::ZERO {
            return None;
        }

        let days = current_il_usd / daily_fees_usd;
        days.to_i32()
    }

    /// Calculate fee velocity (rate of fee accumulation)
    pub fn calculate_fee_velocity(
        &self,
        recent_fees: &[FeeDataPoint],
        window_hours: i64,
    ) -> CalculationResult<Decimal> {
        if recent_fees.len() < 2 {
            return Ok(Decimal::ZERO);
        }

        let cutoff_time = Utc::now() - Duration::hours(window_hours);
        let recent_data: Vec<_> = recent_fees
            .iter()
            .filter(|point| point.timestamp >= cutoff_time)
            .collect();

        if recent_data.len() < 2 {
            return Ok(Decimal::ZERO);
        }

        let first = recent_data.first().unwrap();
        let last = recent_data.last().unwrap();
        
        let time_diff_hours = (last.timestamp - first.timestamp).num_hours();
        if time_diff_hours <= 0 {
            return Ok(Decimal::ZERO);
        }

        let fee_diff = last.fees_usd - first.fees_usd;
        Ok(fee_diff / Decimal::from(time_diff_hours))
    }
}

/// Utility functions for fee calculations
pub fn calculate_fee_apr(
    fees_earned_usd: Decimal,
    position_value_usd: Decimal,
    days_elapsed: i32,
) -> CalculationResult<Decimal> {
    if position_value_usd.is_zero() || days_elapsed <= 0 {
        return Ok(Decimal::ZERO);
    }

    let daily_return = fees_earned_usd / position_value_usd / Decimal::from(days_elapsed);
    Ok(daily_return * Decimal::from(365) * Decimal::from(100))
}

pub fn calculate_compound_fee_apr(
    fees_earned_usd: Decimal,
    position_value_usd: Decimal,
    days_elapsed: i32,
    compounding_frequency: i32, // times per year
) -> CalculationResult<Decimal> {
    if position_value_usd.is_zero() || days_elapsed <= 0 || compounding_frequency <= 0 {
        return Ok(Decimal::ZERO);
    }

    let daily_return = fees_earned_usd / position_value_usd / Decimal::from(days_elapsed);
    let periods_per_year = Decimal::from(compounding_frequency);
    let rate_per_period = daily_return * Decimal::from(365) / periods_per_year;
    
    // APY = (1 + r/n)^n - 1
    let compound_factor = (Decimal::ONE + rate_per_period).powd(periods_per_year).unwrap_or(Decimal::ONE);
    Ok((compound_factor - Decimal::ONE) * Decimal::from(100))
}
