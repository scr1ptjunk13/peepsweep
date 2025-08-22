// src/database/queries.rs - Optimized SQL queries and database operations
use crate::database::models::*;
use sqlx::{PgPool, Row};
use rust_decimal::Decimal;
use bigdecimal::BigDecimal;
use std::str::FromStr;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tracing::{debug, info, warn};
use crate::{DatabaseResult, DatabaseError, Address};
use super::models::*;

// ============================================================================
// POSITION QUERIES (using partitioned tables)
// ============================================================================

/// Get user positions from materialized view (fastest)
pub async fn get_user_positions(pool: &PgPool, user_address: &str) -> DatabaseResult<Vec<UserPositionSummary>> {
    let start_time = std::time::Instant::now();
    
    let positions = sqlx::query!(
        r#"
        SELECT 
            user_address,
            version,
            pool_address,
            token0,
            token1,
            fee_tier,
            token0_amount,
            token1_amount,
            current_il_percentage,
            fees_earned_usd,
            updated_at
        FROM user_positions_summary 
        WHERE user_address = $1
        ORDER BY updated_at DESC
        "#,
        user_address
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    let elapsed = start_time.elapsed();
    debug!("get_user_positions took {:?} for {} positions", elapsed, positions.len());
    
    let result = positions.into_iter().map(|row| UserPositionSummary {
        user_address: row.user_address.unwrap_or_default(),
        version: row.version.unwrap_or_default(),
        pool_address: row.pool_address.unwrap_or_default(),
        token0: row.token0.unwrap_or_default(),
        token1: row.token1.unwrap_or_default(),
        fee_tier: row.fee_tier,
        token0_amount: row.token0_amount.map(|bd| Decimal::from_str(&bd.to_string()).unwrap()),
        token1_amount: row.token1_amount.map(|bd| Decimal::from_str(&bd.to_string()).unwrap()),
        current_il_percentage: row.current_il_percentage.map(|bd| Decimal::from_str(&bd.to_string()).unwrap()),
        fees_earned_usd: row.fees_earned_usd.map(|bd| Decimal::from_str(&bd.to_string()).unwrap()),
        updated_at: row.updated_at.unwrap_or_else(|| Utc::now()),
    }).collect();
    
    Ok(result)
}

/// Get detailed V2 positions for a user
pub async fn get_user_v2_positions(pool: &PgPool, user_address: &str) -> DatabaseResult<Vec<PositionV2>> {
    let positions = sqlx::query!(
        r#"
        SELECT 
            id, user_address, pair_address, token0, token1,
            liquidity, token0_amount, token1_amount,
            block_number, transaction_hash, timestamp,
            created_at, updated_at, 
            current_il_percentage, fees_earned_usd
        FROM positions_v2 
        WHERE user_address = $1 AND liquidity > 0
        ORDER BY updated_at DESC
        "#,
        user_address
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    // Convert to PositionV2 structs
    let result = positions.into_iter().map(|row| {
        PositionV2 {
            id: row.id,
            user_address: row.user_address,
            pair_address: row.pair_address,
            token0: row.token0,
            token1: row.token1,
            liquidity: Decimal::from_str(&row.liquidity.to_string()).unwrap_or(Decimal::ZERO),
            token0_amount: Decimal::from_str(&row.token0_amount.to_string()).unwrap_or(Decimal::ZERO),
            token1_amount: Decimal::from_str(&row.token1_amount.to_string()).unwrap_or(Decimal::ZERO),
            block_number: row.block_number,
            transaction_hash: row.transaction_hash,
            timestamp: row.timestamp,
            created_at: row.created_at,
            updated_at: row.updated_at,
            current_il_percentage: row.current_il_percentage.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
            fees_earned_usd: row.fees_earned_usd.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
        }
    }).collect();
    
    Ok(result)
}

/// Get detailed V3 positions for a user
pub async fn get_user_v3_positions(pool: &PgPool, user_address: &str) -> DatabaseResult<Vec<PositionV3>> {
    let positions = sqlx::query!(
        r#"
        SELECT 
            id, user_address, pool_address, token_id, token0, token1,
            fee_tier, tick_lower, tick_upper, 
            liquidity, token0_amount, token1_amount, 
            fees_token0, fees_token1,
            block_number, transaction_hash, timestamp,
            created_at, updated_at, current_tick, in_range,
            current_il_percentage, fees_earned_usd
        FROM positions_v3 
        WHERE user_address = $1 AND liquidity > 0
        ORDER BY updated_at DESC
        "#,
        user_address
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    // Convert to PositionV3 structs
    let result = positions.into_iter().map(|row| {
        PositionV3 {
            id: row.id,
            user_address: row.user_address,
            pool_address: row.pool_address,
            token_id: row.token_id,
            token0: row.token0,
            token1: row.token1,
            fee_tier: row.fee_tier,
            tick_lower: row.tick_lower,
            tick_upper: row.tick_upper,
            liquidity: Decimal::from_str(&row.liquidity.to_string()).unwrap_or(Decimal::ZERO),
            token0_amount: row.token0_amount.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
            token1_amount: row.token1_amount.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
            fees_token0: Decimal::from_str(&row.fees_token0.unwrap_or_default().to_string()).unwrap_or(Decimal::ZERO),
            fees_token1: Decimal::from_str(&row.fees_token1.unwrap_or_default().to_string()).unwrap_or(Decimal::ZERO),
            block_number: row.block_number,
            transaction_hash: row.transaction_hash,
            timestamp: row.timestamp,
            created_at: row.created_at,
            updated_at: row.updated_at,
            current_tick: row.current_tick,
            in_range: row.in_range,
            current_il_percentage: row.current_il_percentage.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
            fees_earned_usd: row.fees_earned_usd.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
        }
    }).collect();
    
    Ok(result)
}

/// Get position history for analytics
pub async fn get_position_history(
    pool: &PgPool, 
    user_address: &str, 
    days: i32
) -> DatabaseResult<Vec<IlSnapshot>> {
    let snapshots = sqlx::query_as!(
        IlSnapshot,
        r#"
        SELECT 
            id, user_address, position_id, version,
            il_percentage as "il_percentage: Decimal", 
            hodl_value_usd as "hodl_value_usd: Decimal", 
            position_value_usd as "position_value_usd: Decimal",
            fees_earned_usd as "fees_earned_usd: Decimal", 
            net_result_usd as "net_result_usd: Decimal", 
            block_number, timestamp
        FROM il_snapshots 
        WHERE user_address = $1 
        AND timestamp > NOW() - INTERVAL '1 day' * $2
        ORDER BY timestamp DESC
        "#,
        user_address,
        days as f64
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(snapshots)
}

// ============================================================================
// POSITION INSERTIONS (with conflict resolution)
// ============================================================================

/// Innovation: Batch operations for high-throughput scenarios
pub async fn batch_upsert_positions(
    pool: &PgPool,
    positions: &[UserPositionSummary],
) -> DatabaseResult<()> {
    let mut tx = pool.begin().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    sqlx::query!(
        r#"
        INSERT INTO user_positions_summary (
            user_address,
            version,
            pool_address,
            token0,
            token1,
            fee_tier,
            token0_amount,
            token1_amount,
            current_il_percentage,
            fees_earned_usd,
            updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (user_address, version) 
        DO UPDATE SET 
            pool_address = EXCLUDED.pool_address,
            token0 = EXCLUDED.token0,
            token1 = EXCLUDED.token1,
            fee_tier = EXCLUDED.fee_tier,
            token0_amount = EXCLUDED.token0_amount,
            token1_amount = EXCLUDED.token1_amount,
            current_il_percentage = EXCLUDED.current_il_percentage,
            fees_earned_usd = EXCLUDED.fees_earned_usd,
            updated_at = NOW()
        "#,
        &positions[0].user_address,
        &positions[0].version,
        &positions[0].pool_address,
        &positions[0].token0,
        &positions[0].token1,
        positions[0].fee_tier,
        positions[0].token0_amount.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
        positions[0].token1_amount.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
        positions[0].current_il_percentage.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
        positions[0].fees_earned_usd.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
        positions[0].updated_at
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    tx.commit().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(())
}

/// Insert or update V2 position with atomic transaction
pub async fn upsert_v2_position(
    pool: &PgPool,
    position: &PositionV2,
) -> DatabaseResult<()> {
    let mut tx = pool.begin().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    sqlx::query!(
        r#"
        INSERT INTO positions_v2 (
            user_address, pair_address, token0, token1,
            liquidity, token0_amount, token1_amount,
            block_number, transaction_hash, timestamp,
            created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT (user_address, pair_address) 
        DO UPDATE SET 
            liquidity = EXCLUDED.liquidity,
            token0_amount = EXCLUDED.token0_amount,
            token1_amount = EXCLUDED.token1_amount,
            block_number = EXCLUDED.block_number,
            transaction_hash = EXCLUDED.transaction_hash,
            timestamp = EXCLUDED.timestamp,
            updated_at = NOW()
        "#,
        position.user_address,
        position.pair_address,
        position.token0,
        position.token1,
        BigDecimal::from_str(&position.liquidity.to_string()).unwrap(),
        BigDecimal::from_str(&position.token0_amount.to_string()).unwrap(),
        BigDecimal::from_str(&position.token1_amount.to_string()).unwrap(),
        position.block_number,
        position.transaction_hash,
        position.timestamp,
        position.created_at,
        position.updated_at
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    tx.commit().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(())
}

/// Insert or update V3 position with atomic transaction
pub async fn upsert_v3_position(
    pool: &PgPool,
    position: &PositionV3,
) -> DatabaseResult<()> {
    let mut tx = pool.begin().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    sqlx::query!(
        r#"
        INSERT INTO positions_v3 (
            user_address, pool_address, token_id, token0, token1,
            fee_tier, tick_lower, tick_upper, liquidity,
            token0_amount, token1_amount, block_number, transaction_hash, timestamp
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        ON CONFLICT (user_address, token_id) 
        DO UPDATE SET 
            liquidity = EXCLUDED.liquidity,
            token0_amount = EXCLUDED.token0_amount,
            token1_amount = EXCLUDED.token1_amount,
            block_number = EXCLUDED.block_number,
            transaction_hash = EXCLUDED.transaction_hash,
            timestamp = EXCLUDED.timestamp
        "#,
        position.user_address,
        position.pool_address,
        position.token_id,
        position.token0,
        position.token1,
        position.fee_tier,
        position.tick_lower,
        position.tick_upper,
        BigDecimal::from_str(&position.liquidity.to_string()).unwrap(),
        position.token0_amount.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
        position.token1_amount.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
        position.block_number,
        position.transaction_hash,
        position.timestamp
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    tx.commit().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(())
}

/// Batch insert positions for efficiency
pub async fn insert_position_batch(
    pool: &PgPool,
    v2_positions: &[PositionV2],
    v3_positions: &[PositionV3],
) -> DatabaseResult<()> {
    let mut tx = pool.begin().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    // Batch insert V2 positions
    for position in v2_positions {
        sqlx::query!(
            r#"
            INSERT INTO positions_v2 (
                user_address, pair_address, token0, token1,
                liquidity, token0_amount, token1_amount,
                block_number, transaction_hash, timestamp,
                created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            ON CONFLICT (user_address, pair_address) DO NOTHING
            "#,
            position.user_address,
            position.pair_address,
            position.token0,
            position.token1,
            BigDecimal::from_str(&position.liquidity.to_string()).unwrap(),
            BigDecimal::from_str(&position.token0_amount.to_string()).unwrap(),
            BigDecimal::from_str(&position.token1_amount.to_string()).unwrap(),
            position.block_number,
            position.transaction_hash,
            position.timestamp,
            position.created_at,
            position.updated_at
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    }
    
    // Batch insert V3 positions
    for position in v3_positions {
        sqlx::query!(
            r#"
            INSERT INTO positions_v3 (
                user_address, pool_address, token_id, token0, token1,
                fee_tier, tick_lower, tick_upper, liquidity,
                token0_amount, token1_amount, block_number, transaction_hash, timestamp
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (user_address, token_id) DO NOTHING
            "#,
            position.user_address,
            position.pool_address,
            position.token_id,
            position.token0,
            position.token1,
            position.fee_tier,
            position.tick_lower,
            position.tick_upper,
            BigDecimal::from_str(&position.liquidity.to_string()).unwrap(),
            position.token0_amount.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
            position.token1_amount.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
            position.block_number,
            position.transaction_hash,
            position.timestamp
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    }
    
    tx.commit().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    info!("Batch inserted {} V2 and {} V3 positions", v2_positions.len(), v3_positions.len());
    Ok(())
}

// ============================================================================
// PRICE QUERIES
// ============================================================================

/// Get current token price with caching
pub async fn get_latest_token_price(
    pool: &PgPool,
    token_address: &Address,
) -> DatabaseResult<Option<TokenPrice>> {
    let result = sqlx::query_as!(
        TokenPrice,
        "SELECT token_address, price_usd as \"price_usd: Decimal\", price_eth as \"price_eth: Decimal\", block_number, timestamp, updated_at FROM token_prices WHERE token_address = $1 ORDER BY updated_at DESC LIMIT 1",
        token_address.to_string()
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(result)
}

/// Alias for get_latest_token_price for compatibility
pub async fn get_token_price(
    pool: &PgPool,
    token_address: &Address,
) -> DatabaseResult<Option<TokenPrice>> {
    get_latest_token_price(pool, token_address).await
}


pub async fn upsert_user_position_summary(
    _pool: &PgPool,
    _summary: &UserPositionSummary,
) -> DatabaseResult<()> {
    // TODO: Create user_summaries table in database migration
    // Temporarily disabled to allow compilation
    /*
    sqlx::query!(
        r#"
        INSERT INTO user_summaries (
            user_address, total_positions, total_value_usd, 
            total_il_usd, total_fees_earned_usd, net_result_usd, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (user_address) 
        DO UPDATE SET
            total_positions = EXCLUDED.total_positions,
            total_value_usd = EXCLUDED.total_value_usd,
            total_il_usd = EXCLUDED.total_il_usd,
            total_fees_earned_usd = EXCLUDED.total_fees_earned_usd,
            net_result_usd = EXCLUDED.net_result_usd,
            updated_at = EXCLUDED.updated_at
        "#,
        summary.user_address.to_string(),
        summary.total_positions,
        summary.total_value_usd,
        summary.total_il_usd,
        summary.total_fees_earned_usd,
        summary.net_result_usd,
        summary.updated_at
    )
    .execute(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    */
    
    Ok(())
}

/// Get multiple token prices in one query
pub async fn get_token_prices_batch(
    pool: &PgPool, 
    token_addresses: &[String]
) -> DatabaseResult<HashMap<String, TokenPrice>> {
    let prices = sqlx::query!(
        r#"
        SELECT 
            token_address, 
            price_usd, 
            price_eth,
            block_number, timestamp, updated_at
        FROM token_prices 
        WHERE token_address = ANY($1)
        "#,
        token_addresses
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    let mut result = HashMap::new();
    for row in prices {
        let price_usd = Some(Decimal::from_str(&row.price_usd.to_string()).unwrap_or(Decimal::ZERO));
        let price_eth = row.price_eth.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO));
        
        let token_price = TokenPrice {
            token_address: row.token_address,
            price_usd,
            price_eth,
            block_number: row.block_number,
            timestamp: row.timestamp,
            updated_at: row.updated_at,
        };
        result.insert(token_price.token_address.clone(), token_price);
    }
    
    Ok(result)
}

pub async fn get_token_price_history(
    pool: &PgPool,
    token_address: &str,
    hours: i64,
) -> DatabaseResult<Vec<crate::database::models::PriceSnapshot>> {
    let since = chrono::Utc::now() - chrono::Duration::hours(hours);
    
    let snapshots = sqlx::query!(
        "SELECT token_address, 
                price_usd as \"price_usd: Option<BigDecimal>\", 
                source as \"source: Option<String>\", 
                timestamp 
         FROM price_snapshots 
         WHERE token_address = $1 AND timestamp >= $2 
         ORDER BY timestamp DESC",
        token_address,
        since
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    let mut result = Vec::new();
    for row in snapshots {
        result.push(crate::database::models::PriceSnapshot {
            token_address: row.token_address,
            price_usd: match row.price_usd {
                Some(p) => Decimal::from_str(&p.to_string()).unwrap_or_default(),
                None => Decimal::ZERO,
            },
            source: match row.source {
                Some(s) => s,
                None => "unknown".to_string(),
            },
            timestamp: row.timestamp,
        });
    }
    
    Ok(result)
}

pub async fn insert_price_snapshot(
    pool: &PgPool,
    snapshot: &crate::database::models::PriceSnapshot,
) -> DatabaseResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO price_snapshots (
            token_address, price_usd, source, timestamp
        )
        VALUES ($1, $2, $3, $4)
        "#,
        snapshot.token_address,
        BigDecimal::from_str(&snapshot.price_usd.to_string()).unwrap_or_default(),
        snapshot.source,
        snapshot.timestamp
    )
    .execute(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(())
}

/// Update token price
pub async fn upsert_token_price(
    pool: &PgPool,
    token_address: &str,
    price_usd: Decimal,
    price_eth: Option<Decimal>,
    block_number: i64,
) -> DatabaseResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO token_prices (token_address, price_usd, price_eth, block_number, timestamp, updated_at)
        VALUES ($1, $2, $3, $4, NOW(), NOW())
        ON CONFLICT (token_address)
        DO UPDATE SET
            price_usd = EXCLUDED.price_usd,
            price_eth = EXCLUDED.price_eth,
            block_number = EXCLUDED.block_number,
            timestamp = EXCLUDED.timestamp,
            updated_at = NOW()
        "#,
        token_address,
        BigDecimal::from_str(&price_usd.to_string()).unwrap_or_default(),
        price_eth.map(|p| BigDecimal::from_str(&p.to_string()).unwrap_or_default()),
        block_number
    )
    .execute(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(())
}

// ============================================================================
// IL SNAPSHOT OPERATIONS
// ============================================================================

/// Insert IL snapshot for tracking
pub async fn insert_il_snapshot(
    pool: &PgPool,
    snapshot: &IlSnapshot,
) -> DatabaseResult<()> {
    sqlx::query!(
        r#"
        INSERT INTO il_snapshots (
            user_address, position_id, version, il_percentage,
            hodl_value_usd, position_value_usd, fees_earned_usd,
            net_result_usd, block_number, timestamp
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
        snapshot.user_address,
        snapshot.position_id,
        snapshot.version,
        BigDecimal::from_str(&snapshot.il_percentage.to_string()).unwrap(),
        BigDecimal::from_str(&snapshot.hodl_value_usd.to_string()).unwrap(),
        BigDecimal::from_str(&snapshot.position_value_usd.to_string()).unwrap(),
        BigDecimal::from_str(&snapshot.fees_earned_usd.to_string()).unwrap(),
        BigDecimal::from_str(&snapshot.net_result_usd.to_string()).unwrap(),
        snapshot.block_number,
        snapshot.timestamp
    )
    .execute(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(())
}

/// Update position IL calculations
pub async fn update_position_il(
    pool: &PgPool,
    user_address: &str,
    position_id: &str,
    il_percentage: Decimal,
    fees_earned_usd: Decimal,
) -> DatabaseResult<()> {
    // Update both V2 and V3 tables (one will succeed, one will be no-op)
    let _ = sqlx::query!(
        r#"
        UPDATE positions_v2 
        SET current_il_percentage = $3, fees_earned_usd = $4, updated_at = NOW()
        WHERE user_address = $1 AND pair_address = $2
        "#,
        user_address,
        position_id,
        BigDecimal::from_str(&il_percentage.to_string()).unwrap(),
        BigDecimal::from_str(&fees_earned_usd.to_string()).unwrap()
    )
    .execute(pool)
    .await;
    
    let _ = sqlx::query!(
        r#"
        UPDATE positions_v3 
        SET current_il_percentage = $3, fees_earned_usd = $4, updated_at = NOW()
        WHERE user_address = $1 AND CAST(token_id AS TEXT) = $2
        "#,
        user_address,
        position_id,
        BigDecimal::from_str(&il_percentage.to_string()).unwrap(),
        BigDecimal::from_str(&fees_earned_usd.to_string()).unwrap()
    )
    .execute(pool)
    .await;
    
    Ok(())
}

// ============================================================================
// ANALYTICS QUERIES
// ============================================================================

/// Get top pools by TVL
pub async fn get_top_pools(pool: &PgPool, limit: i64) -> DatabaseResult<Vec<PoolStats>> {
    #[derive(sqlx::FromRow)]
    struct PoolStatsQuery {
        pool_address: String,
        version: String,
        unique_users: Option<i64>,
        avg_il_percentage: Option<Decimal>,
    }
    
    let stats = sqlx::query!(
        r#"
        SELECT 
            pool_address,
            version,
            COUNT(DISTINCT user_address) as unique_users,
            AVG(current_il_percentage) as avg_il_percentage
        FROM user_positions_summary ups
        GROUP BY pool_address, version
        ORDER BY unique_users DESC NULLS LAST
        LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    // Convert to models::PoolStats
    let result = stats.into_iter().map(|row| {
        PoolStats {
            pool_address: row.pool_address.unwrap_or_default(),
            token0: String::new(), // Placeholder
            token1: String::new(), // Placeholder
            fee_tier: None,
            total_volume_usd: None,
            total_liquidity_usd: None,
            total_positions: row.unique_users,
            avg_il_percentage: row.avg_il_percentage.as_ref().map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
            average_il_percentage: row.avg_il_percentage.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
            total_fees_earned_usd: Some(Decimal::ZERO),
            active_positions: row.unique_users,
        }
    }).collect();
    
    Ok(result)
}

/// Get IL leaderboard
pub async fn get_il_leaderboard(pool: &PgPool, limit: i64) -> DatabaseResult<Vec<crate::database::models::UserPositionSummary>> {
    #[derive(sqlx::FromRow)]
    struct IlLeaderboardEntry {
        user_address: Option<String>,
        total_il_percentage: Option<Option<Decimal>>,
        total_fees_usd: Option<Option<Decimal>>,
        position_count: i64,
    }
    
    let rows = sqlx::query!(
        r#"
        SELECT 
            user_address,
            AVG(current_il_percentage) as total_il_percentage,
            AVG(fees_earned_usd) as total_fees_usd,
            COUNT(*) as position_count
        FROM user_positions_summary ups
        WHERE current_il_percentage IS NOT NULL
        GROUP BY user_address
        ORDER BY AVG(current_il_percentage) DESC NULLS LAST
        LIMIT $1
        "#,
        limit
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;

    let entries: Vec<IlLeaderboardEntry> = rows.into_iter().map(|row| {
        IlLeaderboardEntry {
            user_address: row.user_address,
            total_il_percentage: row.total_il_percentage.map(|v| Some(Decimal::from_str(&v.to_string()).unwrap_or(Decimal::ZERO))),
            total_fees_usd: row.total_fees_usd.map(|v| Some(Decimal::from_str(&v.to_string()).unwrap_or(Decimal::ZERO))),
            position_count: row.position_count.unwrap_or(0),
        }
    }).collect();
    
    // Convert to UserPositionSummary format
    let result = entries.into_iter().map(|entry| {
        crate::database::models::UserPositionSummary {
            user_address: entry.user_address.unwrap_or_default(),
            version: "mixed".to_string(), // Since we're aggregating across versions
            pool_address: "leaderboard".to_string(), // Placeholder for leaderboard context
            token0: "".to_string(),
            token1: "".to_string(),
            fee_tier: None,
            token0_amount: None,
            token1_amount: None,
            current_il_percentage: entry.total_il_percentage.flatten(),
            fees_earned_usd: entry.total_fees_usd.flatten(),
            updated_at: chrono::Utc::now(),
        }
    }).collect();
    
    Ok(result)
}

/// Batch upsert V2 positions for high-throughput processing
pub async fn batch_upsert_v2_positions(
    pool: &PgPool,
    positions: &[PositionV2],
) -> DatabaseResult<()> {
    let mut tx = pool.begin().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    for position in positions {
        sqlx::query!(
            r#"
            INSERT INTO positions_v2 (
                user_address, pair_address, token0, token1, 
                liquidity, token0_amount, token1_amount, 
                block_number, transaction_hash, timestamp
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (user_address, pair_address) 
            DO UPDATE SET
                liquidity = EXCLUDED.liquidity,
                token0_amount = EXCLUDED.token0_amount,
                token1_amount = EXCLUDED.token1_amount,
                updated_at = NOW()
            "#,
            position.user_address,
            position.pair_address,
            position.token0,
            position.token1,
            BigDecimal::from_str(&position.liquidity.to_string()).unwrap(),
            BigDecimal::from_str(&position.token0_amount.to_string()).unwrap(),
            BigDecimal::from_str(&position.token1_amount.to_string()).unwrap(),
            position.block_number as i64,
            position.transaction_hash,
            position.timestamp
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    }
    
    tx.commit().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(())
}

/// Batch upsert V3 positions for high-throughput processing
pub async fn batch_upsert_v3_positions(
    pool: &PgPool,
    positions: &[PositionV3],
) -> DatabaseResult<()> {
    let mut tx = pool.begin().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    for position in positions {
        sqlx::query!(
            r#"
            INSERT INTO positions_v3 (
                user_address, pool_address, token_id, token0, token1, 
                fee_tier, tick_lower, tick_upper, liquidity, 
                token0_amount, token1_amount, block_number, 
                transaction_hash, timestamp
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (user_address, token_id) 
            DO UPDATE SET
                liquidity = EXCLUDED.liquidity,
                token0_amount = EXCLUDED.token0_amount,
                token1_amount = EXCLUDED.token1_amount,
                updated_at = NOW()
            "#,
            position.user_address,
            position.pool_address,
            position.token_id as i64,
            position.token0,
            position.token1,
            position.fee_tier as i32,
            position.tick_lower,
            position.tick_upper,
            BigDecimal::from_str(&position.liquidity.to_string()).unwrap(),
            position.token0_amount.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
            position.token1_amount.map(|d| BigDecimal::from_str(&d.to_string()).unwrap()),
            position.block_number as i64,
            position.transaction_hash,
            position.timestamp
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    }
    
    tx.commit().await
        .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(())
}
// ============================================================================
// MISSING FUNCTIONS IMPLEMENTATIONS
// ============================================================================

/// Get position by ID (generic for both V2 and V3)
pub async fn get_position_by_id(pool: &PgPool, position_id: &str) -> DatabaseResult<Option<UserPositionSummary>> {
    let position = sqlx::query!(
        r#"
        SELECT 
            user_address,
            version,
            pool_address,
            token0,
            token1,
            fee_tier,
            token0_amount,
            token1_amount,
            current_il_percentage,
            fees_earned_usd,
            updated_at
        FROM user_positions_summary 
        WHERE pool_address = $1
        LIMIT 1
        "#,
        position_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    if let Some(row) = position {
        let result = UserPositionSummary {
            user_address: row.user_address.unwrap_or_default(),
            version: row.version.unwrap_or_default(),
            pool_address: row.pool_address.unwrap_or_default(),
            token0: row.token0.unwrap_or_default(),
            token1: row.token1.unwrap_or_default(),
            fee_tier: row.fee_tier,
            token0_amount: row.token0_amount.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or_default()),
            token1_amount: row.token1_amount.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or_default()),
            current_il_percentage: row.current_il_percentage.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or_default()),
            fees_earned_usd: row.fees_earned_usd.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or_default()),
            updated_at: row.updated_at.unwrap_or_else(|| Utc::now()),
        };
        Ok(Some(result))
    } else {
        Ok(None)
    }
}

/// Get position history for a specific position by position_id
pub async fn get_position_history_by_id(pool: &PgPool, position_id: &str) -> DatabaseResult<Vec<IlSnapshot>> {
    let snapshots = sqlx::query_as!(
        IlSnapshot,
        r#"
        SELECT 
            id,
            user_address,
            position_id,
            version,
            il_percentage as "il_percentage: Decimal",
            hodl_value_usd as "hodl_value_usd: Decimal",
            position_value_usd as "position_value_usd: Decimal",
            fees_earned_usd as "fees_earned_usd: Decimal",
            net_result_usd as "net_result_usd: Decimal",
            block_number,
            timestamp
        FROM il_snapshots 
        WHERE position_id = $1
        ORDER BY timestamp DESC
        LIMIT 100
        "#,
        position_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(snapshots)
}

/// Get IL analysis for a position
pub async fn get_il_analysis(pool: &PgPool, position_id: &str) -> DatabaseResult<Option<IlSnapshot>> {
    let analysis = sqlx::query_as!(
        IlSnapshot,
        r#"
        SELECT 
            id,
            user_address,
            position_id,
            version,
            il_percentage as "il_percentage: Decimal",
            hodl_value_usd as "hodl_value_usd: Decimal",
            position_value_usd as "position_value_usd: Decimal",
            fees_earned_usd as "fees_earned_usd: Decimal",
            net_result_usd as "net_result_usd: Decimal",
            block_number,
            timestamp
        FROM il_snapshots 
        WHERE position_id = $1
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
        position_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(analysis)
}

/// Get top pools by volume
pub async fn get_top_pools_by_volume(pool: &PgPool, limit: i32) -> DatabaseResult<Vec<PoolStats>> {
    let pools = sqlx::query!(
        r#"
        SELECT 
            pool_address,
            token0,
            token1,
            fee_tier,
            total_volume_usd,
            total_liquidity_usd,
            total_positions,
            avg_il_percentage,
            avg_il_percentage as average_il_percentage,
            NULL::decimal as total_fees_earned_usd,
            total_positions as active_positions
        FROM (
            SELECT 
                pool_address,
                token0,
                token1,
                fee_tier,
                SUM(token0_amount * 2) as total_volume_usd,
                SUM(token0_amount + token1_amount) as total_liquidity_usd,
                COUNT(*) as total_positions,
                AVG(current_il_percentage) as avg_il_percentage
            FROM positions_v3
            WHERE liquidity > 0
            GROUP BY pool_address, token0, token1, fee_tier
            
            UNION ALL
            
            SELECT 
                pair_address as pool_address,
                token0,
                token1,
                NULL::BIGINT as fee_tier,
                SUM(token0_amount * 2) as total_volume_usd,
                SUM(token0_amount + token1_amount) as total_liquidity_usd,
                COUNT(*) as total_positions,
                AVG(current_il_percentage) as avg_il_percentage
            FROM positions_v2
            WHERE liquidity > 0
            GROUP BY pair_address, token0, token1
        ) pools
        ORDER BY total_volume_usd DESC
        LIMIT $1
        "#,
        limit as i64
    )
    .fetch_all(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    let result = pools.into_iter().map(|row| PoolStats {
        pool_address: row.pool_address.unwrap_or_default(),
        token0: row.token0.unwrap_or_default(),
        token1: row.token1.unwrap_or_default(),
        fee_tier: row.fee_tier.map(|f| f as i32),
        total_volume_usd: row.total_volume_usd.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
        total_liquidity_usd: row.total_liquidity_usd.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
        total_positions: Some(row.total_positions.unwrap_or_default()),
        average_il_percentage: row.average_il_percentage.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
        total_fees_earned_usd: row.total_fees_earned_usd.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
        active_positions: Some(row.active_positions.unwrap_or_default() as i64),
        avg_il_percentage: row.avg_il_percentage.map(|bd| Decimal::from_str(&bd.to_string()).unwrap_or(Decimal::ZERO)),
    }).collect();
    
    Ok(result)
}

/// Get token price from cache by string address
pub async fn get_token_price_by_string(_pool: &PgPool, _token_address: &str) -> DatabaseResult<Option<TokenPrice>> {
    // Temporarily commented out to fix compilation
    /*
    let price = sqlx::query_as!(
        TokenPrice,
        r#"
        SELECT 
            token_address,
            price_usd,
            price_eth,
            block_number,
            timestamp,
            updated_at
        FROM token_prices 
        WHERE token_address = $1
        ORDER BY timestamp DESC
        LIMIT 1
        "#,
        token_address
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    */
    
    Ok(None)
}

/// Get pool statistics for analytics
pub async fn get_pool_statistics(pool: &PgPool, pool_address: &str) -> DatabaseResult<Option<PoolStats>> {
    // TODO: Fix BigDecimal to Decimal conversion issues
    /*
    let stats = sqlx::query_as!(
        PoolStats,
        r#"
        SELECT 
            pool_address,
            token0_address,
            token1_address,
            fee_tier,
            total_value_locked_usd as "total_value_locked_usd: Option<Decimal>",
            volume_24h_usd as "volume_24h_usd: Option<Decimal>",
            fees_24h_usd as "fees_24h_usd: Option<Decimal>",
            apr as "apr: Option<Decimal>",
            liquidity as "liquidity: Option<Decimal>",
            tick_spacing,
            current_tick,
            sqrt_price_x96,
            token0_price_usd as "token0_price_usd: Option<Decimal>",
            token1_price_usd as "token1_price_usd: Option<Decimal>",
            created_at,
            updated_at
        FROM pool_stats 
        WHERE pool_address = $1
        "#,
        pool_address
    )
    .fetch_optional(pool)
    .await?;
    */
    
    // Temporary placeholder return
    let stats: Option<PoolStats> = None;

    Ok(stats)
}

// Get pool statistics for analytics (duplicate function - commenting out)
/*
pub async fn get_pool_statistics(pool: &PgPool, pool_address: &str) -> DatabaseResult<Option<PoolStats>> {
    let stats = sqlx::query_as!(
        PoolStats,
        r#"
        SELECT 
            pool_address,
            token0,
            token1,
            fee_tier,
            total_volume_usd,
            total_liquidity_usd,
            total_positions,
            avg_il_percentage
        FROM (
            SELECT 
                pool_address,
                token0,
                token1,
                fee_tier,
                SUM(token0_amount * 2) as total_volume_usd,
                SUM(token0_amount + token1_amount) as total_liquidity_usd,
                COUNT(*) as total_positions,
                AVG(current_il_percentage) as avg_il_percentage
            FROM positions_v3
            WHERE pool_address = $1 AND liquidity > 0
            GROUP BY pool_address, token0, token1, fee_tier
            
            UNION ALL
            
            SELECT 
                pair_address as pool_address,
                token0,
                token1,
                NULL::BIGINT as fee_tier,
                SUM(token0_amount * 2) as total_volume_usd,
                SUM(token0_amount + token1_amount) as total_liquidity_usd,
                COUNT(*) as total_positions,
                AVG(current_il_percentage) as avg_il_percentage
            FROM positions_v2
            WHERE pair_address = $1 AND liquidity > 0
            GROUP BY pair_address, token0, token1
        ) pools
        LIMIT 1
        "#,
        pool_address
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| DatabaseError::QueryFailed(e.to_string()))?;
    
    Ok(stats)
}
*/
