use redis::{Client, RedisError};
use serde::Serialize;
use tracing::{error, info};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct CacheManager {
    client: Client,
    _connection: Arc<Mutex<()>>,
}

impl CacheManager {
    pub async fn new(redis_url: &str) -> Result<Self, RedisError> {
        let client = Client::open(redis_url)?;
        
        Ok(CacheManager {
            client,
            _connection: Arc::new(Mutex::new(())),
        })
    }

    pub async fn get_connection(&self) -> anyhow::Result<redis::Connection> {
        self.client.get_connection()
            .map_err(|e| anyhow::anyhow!("Redis connection error: {}", e))
    }

    // Cache quote for 30 seconds to balance freshness vs performance
    pub async fn cache_quote<T>(&self, key: &str, value: &T, ttl_seconds: u64) -> redis::RedisResult<()>
    where
        T: Serialize,
    {
        match self.get_connection().await {
            Ok(mut conn) => {
                let serialized = serde_json::to_string(value).map_err(|e| {
                    error!("Serialization error: {}", e);
                    redis::RedisError::from((redis::ErrorKind::TypeError, "Serialization failed"))
                })?;
                
                let _: () = redis::cmd("SET").arg(key).arg(serialized).query(&mut conn)?;
                let _: () = redis::cmd("EXPIRE").arg(key).arg(ttl_seconds).query(&mut conn)?;
                info!("Cached quote with key: {}", key);
                Ok(())
            }
            Err(e) => {
                error!("Redis connection error: {}", e);
                Err(redis::RedisError::from((redis::ErrorKind::IoError, "Connection failed")))
            }
        }
    }

    pub async fn get_cached_quote<T>(&self, key: &str) -> anyhow::Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        // Fast cache lookup with timeout
        let mut conn = self.client.get_connection()?;
        
        match redis::cmd("GET").arg(key).query::<String>(&mut conn) {
            Ok(cached_data) => {
                let quote: T = serde_json::from_str(&cached_data)
                    .map_err(|e| anyhow::anyhow!("Deserialization error: {}", e))?;
                info!("Cache hit for key: {}", key);
                Ok(Some(quote))
            }
            Err(_) => {
                info!("Cache miss for key: {}", key);
                Ok(None)
            }
        }
    }

    pub fn generate_cache_key(token_in: &str, token_out: &str, amount_in: &str) -> String {
        format!("quote:{}:{}:{}", token_in, token_out, amount_in)
    }

    // Cache gas prices for 60 seconds
    pub async fn cache_gas_price(&self, gas_price: u64) -> redis::RedisResult<()> {
        match self.get_connection().await {
            Ok(mut conn) => {
                let _: () = redis::cmd("SET").arg("gas_price").arg(gas_price).query(&mut conn)?;
                let _: () = redis::cmd("EXPIRE").arg("gas_price").arg(60).query(&mut conn)?;
                Ok(())
            }
            Err(_e) => {
                error!("Failed to cache gas price");
                Err(redis::RedisError::from((redis::ErrorKind::IoError, "Connection failed")))
            }
        }
    }

    pub async fn get_cached_gas_price(&self) -> Option<u64> {
        match self.get_connection().await {
            Ok(mut conn) => {
                match redis::cmd("GET").arg("gas_price").query::<u64>(&mut conn) {
                    Ok(gas_price) => Some(gas_price),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }

    pub fn get_redis_client(&self) -> Client {
        self.client.clone()
    }
}
