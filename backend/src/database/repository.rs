use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;
use anyhow::Result;
use tracing::{info, error, warn};
use chrono::{DateTime, Utc};

use super::models::*;

pub struct TokenRepository {
    pool: PgPool,
}

impl TokenRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // Core token operations
    pub async fn create_token(&self, new_token: NewToken) -> Result<Uuid> {
        let token_id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO tokens (id, symbol, name, coingecko_id, token_type, decimals, 
                              total_supply, is_verified, verification_level, description,
                              website_url, twitter_handle, telegram_url, discord_url)
            VALUES ($1, $2, $3, $4, $5, $6, $7::numeric, $8, $9, $10, $11, $12, $13, $14)
            "#)
            .bind(token_id)
            .bind(&new_token.symbol)
            .bind(&new_token.name)
            .bind(&new_token.coingecko_id)
            .bind(&new_token.token_type)
            .bind(new_token.decimals)
            .bind(new_token.total_supply.map(|d| d.to_string()))
            .bind(new_token.is_verified)
            .bind(&new_token.verification_level)
            .bind(&new_token.description)
            .bind(&new_token.website_url)
            .bind(&new_token.twitter_handle)
            .bind(&new_token.telegram_url)
            .bind(&new_token.discord_url)
        .execute(&self.pool)
        .await?;

        info!("Created token: {} ({})", new_token.symbol, token_id);
        Ok(token_id)
    }

    pub async fn add_token_address(&self, new_address: NewTokenAddress) -> Result<Uuid> {
        let address_id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO token_addresses (id, token_id, chain_id, address, is_native, 
                                       is_wrapped, proxy_address, implementation_address)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#)
            .bind(address_id)
            .bind(new_address.token_id)
            .bind(new_address.chain_id)
            .bind(&new_address.address)
            .bind(new_address.is_native)
            .bind(new_address.is_wrapped)
            .bind(&new_address.proxy_address)
            .bind(&new_address.implementation_address)
        .execute(&self.pool)
        .await?;

        Ok(address_id)
    }

    pub async fn upsert_market_data(&self, market_data: NewTokenMarketData) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO token_market_data (token_id, price_usd, market_cap_usd, volume_24h_usd,
                                         volume_7d_usd, price_change_24h, price_change_7d,
                                         circulating_supply, max_supply, ath_usd, atl_usd,
                                         liquidity_usd, holders_count)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            ON CONFLICT (token_id) DO UPDATE SET
                price_usd = EXCLUDED.price_usd,
                market_cap_usd = EXCLUDED.market_cap_usd,
                volume_24h_usd = EXCLUDED.volume_24h_usd,
                volume_7d_usd = EXCLUDED.volume_7d_usd,
                price_change_24h = EXCLUDED.price_change_24h,
                price_change_7d = EXCLUDED.price_change_7d,
                circulating_supply = EXCLUDED.circulating_supply,
                max_supply = EXCLUDED.max_supply,
                ath_usd = EXCLUDED.ath_usd,
                atl_usd = EXCLUDED.atl_usd,
                liquidity_usd = EXCLUDED.liquidity_usd,
                holders_count = EXCLUDED.holders_count,
                last_updated = NOW()
            "#)
            .bind(market_data.token_id)
            .bind(market_data.price_usd.map(|d| d.to_string()))
            .bind(market_data.market_cap_usd.map(|d| d.to_string()))
            .bind(market_data.volume_24h_usd.map(|d| d.to_string()))
            .bind(market_data.volume_7d_usd.map(|d| d.to_string()))
            .bind(market_data.price_change_24h.map(|d| d.to_string()))
            .bind(market_data.price_change_7d.map(|d| d.to_string()))
            .bind(market_data.circulating_supply.map(|d| d.to_string()))
            .bind(market_data.max_supply.map(|d| d.to_string()))
            .bind(market_data.ath_usd.map(|d| d.to_string()))
            .bind(market_data.atl_usd.map(|d| d.to_string()))
            .bind(market_data.liquidity_usd.map(|d| d.to_string()))
            .bind(market_data.holders_count)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_token_source(&self, source: NewTokenSource) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO token_sources (token_id, source_name, source_priority, metadata)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (token_id, source_name) DO UPDATE SET
                source_priority = EXCLUDED.source_priority,
                metadata = EXCLUDED.metadata,
                last_seen_at = NOW(),
                is_active = true
            "#)
            .bind(source.token_id)
            .bind(&source.source_name)
            .bind(source.source_priority)
            .bind(&source.metadata)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Fast token lookups
    pub async fn get_unified_tokens(&self, limit: Option<i64>, offset: Option<i64>) -> Result<Vec<UnifiedToken>> {
        let limit = limit.unwrap_or(1000);
        let offset = offset.unwrap_or(0);

        let rows = sqlx::query(
            r#"
            SELECT 
                t.id,
                t.symbol,
                t.name,
                t.token_type::text,
                t.decimals,
                t.is_verified,
                t.verification_level::text,
                tl.logo_url,
                tl.cdn_url,
                tm.price_usd,
                tm.market_cap_usd,
                tm.volume_24h_usd,
                t.updated_at
            FROM tokens t
            LEFT JOIN token_logos tl ON t.id = tl.token_id
            LEFT JOIN token_market_data tm ON t.id = tm.token_id
            ORDER BY t.verification_level DESC, tm.volume_24h_usd DESC NULLS LAST
            LIMIT $1 OFFSET $2
            "#)
            .bind(limit)
            .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut tokens = Vec::new();
        for row in rows {
            let token_id: Uuid = row.get("id");
            let symbol: String = row.get("symbol");
            let name: String = row.get("name");
            let decimals: i32 = row.get("decimals");
            let is_verified: bool = row.get("is_verified");
            
            let mut token = UnifiedToken {
                id: token_id,
                symbol,
                name,
                token_type: TokenType::ERC20, // Default to ERC20
                decimals,
                is_verified,
                verification_level: VerificationLevel::Unverified, // Default verification level
                chain_addresses: HashMap::new(),
                logo_url: row.get("logo_url"),
                cdn_url: None, // Default to None
                price_usd: row.get::<Option<String>, _>("price_usd").and_then(|s| s.parse().ok()),
                market_cap_usd: row.get::<Option<String>, _>("market_cap_usd").and_then(|s| s.parse().ok()),
                volume_24h_usd: row.get::<Option<String>, _>("volume_24h_usd").and_then(|s| s.parse().ok()),
                market_data: None, // Default to None
                security: None, // Default to None
                tags: Vec::new(),
                sources: Vec::new(),
                updated_at: Utc::now(), // Set current timestamp
            };
            
            // Get chain addresses separately
            token.chain_addresses = self.get_token_addresses(token_id).await?;
            
            tokens.push(token);
        }

        Ok(tokens)
    }

    pub async fn get_token_by_symbol(&self, symbol: &str) -> Result<Option<UnifiedToken>> {
        let row = sqlx::query(
            r#"
            SELECT 
                t.id,
                t.symbol,
                t.name,
                t.decimals,
                t.is_verified,
                tl.logo_url,
                tm.price_usd,
                tm.market_cap_usd,
                tm.volume_24h_usd
            FROM tokens t
            LEFT JOIN token_logos tl ON t.id = tl.token_id
            LEFT JOIN token_market_data tm ON t.id = tm.token_id
            WHERE UPPER(t.symbol) = UPPER($1)
            ORDER BY t.verification_level DESC
            LIMIT 1
            "#)
            .bind(symbol)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let token_id: Uuid = row.get("id");
            let token = UnifiedToken {
                id: token_id,
                symbol: row.get("symbol"),
                name: row.get("name"),
                token_type: TokenType::ERC20,
                decimals: row.get("decimals"),
                is_verified: row.get("is_verified"),
                verification_level: VerificationLevel::Unverified,
                chain_addresses: self.get_token_addresses(token_id).await?,
                logo_url: row.get("logo_url"),
                cdn_url: None,
                price_usd: row.get::<Option<String>, _>("price_usd").and_then(|s| s.parse().ok()),
                market_cap_usd: row.get::<Option<String>, _>("market_cap_usd").and_then(|s| s.parse().ok()),
                volume_24h_usd: row.get::<Option<String>, _>("volume_24h_usd").and_then(|s| s.parse().ok()),
                market_data: None,
                security: None,
                tags: Vec::new(),
                sources: Vec::new(),
                updated_at: Utc::now(),
            };
            Ok(Some(token))
        } else {
            Ok(None)
        }
    }

    pub async fn get_tokens_by_chain(&self, chain_id: i64) -> Result<Vec<UnifiedToken>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT
                t.id,
                t.symbol,
                t.name,
                t.decimals,
                t.is_verified,
                tl.logo_url,
                tm.price_usd,
                tm.market_cap_usd,
                tm.volume_24h_usd
            FROM tokens t
            INNER JOIN token_addresses ta ON t.id = ta.token_id
            LEFT JOIN token_logos tl ON t.id = tl.token_id
            LEFT JOIN token_market_data tm ON t.id = tm.token_id
            WHERE ta.chain_id = $1
            ORDER BY t.verification_level DESC, tm.volume_24h_usd DESC NULLS LAST
            "#)
            .bind(chain_id)
        .fetch_all(&self.pool)
        .await?;

        let mut tokens = Vec::new();
        for row in rows {
            let token_id: Uuid = row.get("id");
            let token = UnifiedToken {
                id: token_id,
                symbol: row.get("symbol"),
                name: row.get("name"),
                token_type: TokenType::ERC20,
                decimals: row.get("decimals"),
                is_verified: row.get("is_verified"),
                verification_level: VerificationLevel::Unverified,
                chain_addresses: self.get_token_addresses(token_id).await?,
                logo_url: row.get("logo_url"),
                cdn_url: None,
                price_usd: row.get::<Option<String>, _>("price_usd").and_then(|s| s.parse().ok()),
                market_cap_usd: row.get::<Option<String>, _>("market_cap_usd").and_then(|s| s.parse().ok()),
                volume_24h_usd: row.get::<Option<String>, _>("volume_24h_usd").and_then(|s| s.parse().ok()),
                market_data: None,
                security: None,
                tags: Vec::new(),
                sources: Vec::new(),
                updated_at: Utc::now(),
            };
            tokens.push(token);
        }

        Ok(tokens)
    }

    pub async fn search_tokens(&self, query: &str, limit: Option<i64>) -> Result<Vec<UnifiedToken>> {
        let limit = limit.unwrap_or(50);
        
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT
                t.id,
                t.symbol,
                t.name,
                t.decimals,
                t.is_verified,
                tl.logo_url,
                tm.price_usd,
                tm.market_cap_usd,
                tm.volume_24h_usd
            FROM tokens t
            LEFT JOIN token_logos tl ON t.id = tl.token_id
            LEFT JOIN token_market_data tm ON t.id = tm.token_id
            WHERE 
                UPPER(t.symbol) LIKE UPPER($1) OR
                UPPER(t.name) LIKE UPPER($1)
            ORDER BY 
                CASE WHEN UPPER(t.symbol) = UPPER($2) THEN 1 ELSE 2 END,
                t.verification_level DESC,
                tm.volume_24h_usd DESC NULLS LAST
            LIMIT $3
            "#)
            .bind(format!("%{}%", query))
            .bind(query)
            .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let mut tokens = Vec::new();
        for row in rows {
            let token_id: Uuid = row.get("id");
            let token = UnifiedToken {
                id: token_id,
                symbol: row.get("symbol"),
                name: row.get("name"),
                token_type: TokenType::ERC20,
                decimals: row.get("decimals"),
                is_verified: row.get("is_verified"),
                verification_level: VerificationLevel::Unverified,
                chain_addresses: self.get_token_addresses(token_id).await?,
                logo_url: row.get("logo_url"),
                cdn_url: None,
                price_usd: row.get::<Option<String>, _>("price_usd").and_then(|s| s.parse().ok()),
                market_cap_usd: row.get::<Option<String>, _>("market_cap_usd").and_then(|s| s.parse().ok()),
                volume_24h_usd: row.get::<Option<String>, _>("volume_24h_usd").and_then(|s| s.parse().ok()),
                market_data: None,
                security: None,
                tags: Vec::new(),
                sources: Vec::new(),
                updated_at: Utc::now(),
            };
            tokens.push(token);
        }

        Ok(tokens)
    }

    // Helper methods
    async fn get_token_tags(&self, token_id: Uuid) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT tag FROM token_tags WHERE token_id = $1")
            .bind(token_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| row.get("tag")).collect())
    }

    async fn get_token_sources(&self, token_id: Uuid) -> Result<Vec<String>> {
        let rows = sqlx::query(
            "SELECT source_name FROM token_sources WHERE token_id = $1 AND is_active = true")
            .bind(token_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(|row| row.get("source_name")).collect())
    }

    async fn get_token_addresses(&self, token_id: Uuid) -> Result<HashMap<i64, String>> {
        let rows = sqlx::query(
            "SELECT chain_id, address FROM token_addresses WHERE token_id = $1")
            .bind(token_id)
        .fetch_all(&self.pool)
        .await?;

        let mut addresses = HashMap::new();
        for row in rows {
            let chain_id: i64 = row.get("chain_id");
            let address: String = row.get("address");
            addresses.insert(chain_id, address);
        }
        Ok(addresses)
    }

    // Batch operations for performance
    pub async fn upsert_tokens_batch(&self, tokens_data: Vec<(NewToken, Vec<NewTokenAddress>)>) -> Result<Vec<Uuid>> {
        let mut token_ids = Vec::new();
        
        for (new_token, addresses) in tokens_data {
            // Check if token exists by symbol
            let existing_token = sqlx::query(
                "SELECT id FROM tokens WHERE UPPER(symbol) = UPPER($1) LIMIT 1")
                .bind(&new_token.symbol)
            .fetch_optional(&self.pool)
            .await?;

            let token_id = if let Some(existing) = existing_token {
                existing.get("id")
            } else {
                self.create_token(new_token).await?
            };

            // Add all addresses for this token
            for address in addresses {
                let mut addr = address;
                addr.token_id = token_id;
                self.add_token_address(addr).await?;
            }

            token_ids.push(token_id);
        }

        Ok(token_ids)
    }

    // Job tracking
    pub async fn create_discovery_job(&self, job_type: &str, chain_id: Option<i64>, source_name: Option<String>) -> Result<Uuid> {
        let job_id = Uuid::new_v4();
        
        sqlx::query(
            r#"
            INSERT INTO discovery_jobs (id, job_type, chain_id, source_name, status, started_at)
            VALUES ($1, $2, $3, $4, 'Running', NOW())
            "#)
            .bind(job_id)
            .bind(job_type)
            .bind(chain_id)
            .bind(source_name)
        .execute(&self.pool)
        .await?;

        Ok(job_id)
    }

    pub async fn complete_discovery_job(&self, job_id: Uuid, tokens_processed: i32, tokens_added: i32, tokens_updated: i32) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE discovery_jobs 
            SET status = 'Completed', completed_at = NOW(), 
                tokens_processed = $2, tokens_added = $3, tokens_updated = $4
            WHERE id = $1
            "#)
            .bind(job_id)
            .bind(tokens_processed)
            .bind(tokens_added)
            .bind(tokens_updated)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn fail_discovery_job(&self, job_id: Uuid, error_message: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE discovery_jobs 
            SET status = 'Failed', completed_at = NOW(), error_message = $2
            WHERE id = $1
            "#)
            .bind(job_id)
            .bind(error_message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // Statistics
    pub async fn get_token_count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM tokens")
            .fetch_one(&self.pool)
            .await?;
        
        Ok(row.get::<i64, _>("count"))
    }

    pub async fn get_chain_token_counts(&self) -> Result<HashMap<i64, i64>> {
        let rows = sqlx::query(
            r#"
            SELECT ta.chain_id, COUNT(DISTINCT ta.token_id) as token_count
            FROM token_addresses ta
            GROUP BY ta.chain_id
            "#)
        .fetch_all(&self.pool)
        .await?;

        let mut counts = HashMap::new();
        for row in rows {
            let chain_id: i64 = row.get("chain_id");
            let token_count: i64 = row.get("token_count");
            counts.insert(chain_id, token_count);
        }

        Ok(counts)
    }
}
