use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use super::{Message, GenerateOptions, GenerateResponse};

/// Configuration for response caching
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Time-to-live for cache entries
    pub ttl: Duration,
    /// Maximum number of entries in the cache
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            ttl: Duration::from_secs(3600), // 1 hour
            max_entries: 1000,
        }
    }
}

impl CacheConfig {
    /// Create a new cache configuration
    pub fn new(enabled: bool, ttl: Duration, max_entries: usize) -> Self {
        Self {
            enabled,
            ttl,
            max_entries,
        }
    }

    /// Create a configuration with caching disabled
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ttl: Duration::from_secs(0),
            max_entries: 0,
        }
    }

    /// Create a configuration with short TTL (5 minutes)
    pub fn short_lived() -> Self {
        Self {
            enabled: true,
            ttl: Duration::from_secs(300),
            max_entries: 100,
        }
    }

    /// Create a configuration with long TTL (24 hours)
    pub fn long_lived() -> Self {
        Self {
            enabled: true,
            ttl: Duration::from_secs(86400),
            max_entries: 10000,
        }
    }
}

/// Key for caching responses
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    messages_hash: u64,
    model: String,
    options_hash: u64,
}

impl CacheKey {
    /// Create a cache key from request parameters
    pub fn from_request(
        messages: &[Message],
        model: &str,
        options: &Option<GenerateOptions>,
    ) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();

        // Hash messages
        for msg in messages {
            format!("{:?}:{}", msg.role, msg.content_as_text()).hash(&mut hasher);
        }
        let messages_hash = hasher.finish();

        // Hash options
        let mut options_hasher = std::collections::hash_map::DefaultHasher::new();
        if let Some(opts) = options {
            if let Some(temp) = opts.temperature {
                temp.to_bits().hash(&mut options_hasher);
            }
            if let Some(max_tokens) = opts.max_tokens {
                max_tokens.hash(&mut options_hasher);
            }
            if let Some(top_p) = opts.top_p {
                top_p.to_bits().hash(&mut options_hasher);
            }
            if let Some(stop) = &opts.stop {
                stop.hash(&mut options_hasher);
            }
        }
        let options_hash = options_hasher.finish();

        Self {
            messages_hash,
            model: model.to_string(),
            options_hash,
        }
    }
}

/// Entry in the cache
#[derive(Debug, Clone)]
struct CacheEntry {
    response: GenerateResponse,
    created_at: Instant,
    access_count: usize,
}

impl CacheEntry {
    fn new(response: GenerateResponse) -> Self {
        Self {
            response,
            created_at: Instant::now(),
            access_count: 0,
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

/// Response cache with TTL and LRU eviction
pub struct ResponseCache {
    config: CacheConfig,
    entries: Arc<RwLock<HashMap<CacheKey, CacheEntry>>>,
    stats: Arc<RwLock<CacheStats>>,
}

impl ResponseCache {
    /// Create a new response cache with the given configuration
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            entries: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    /// Get a cached response if available and not expired
    pub async fn get(&self, key: &CacheKey) -> Option<GenerateResponse> {
        if !self.config.enabled {
            return None;
        }

        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get_mut(key) {
            if entry.is_expired(self.config.ttl) {
                entries.remove(key);
                let mut stats = self.stats.write().await;
                stats.misses += 1;
                None
            } else {
                entry.access_count += 1;
                let mut stats = self.stats.write().await;
                stats.hits += 1;
                Some(entry.response.clone())
            }
        } else {
            let mut stats = self.stats.write().await;
            stats.misses += 1;
            None
        }
    }

    /// Store a response in the cache
    pub async fn put(&self, key: CacheKey, response: GenerateResponse) {
        if !self.config.enabled {
            return;
        }

        let mut entries = self.entries.write().await;

        // Evict expired entries
        entries.retain(|_, entry| !entry.is_expired(self.config.ttl));

        // Evict least recently used entries if at capacity
        if entries.len() >= self.config.max_entries {
            self.evict_lru(&mut entries).await;
        }

        entries.insert(key, CacheEntry::new(response));
    }

    /// Evict the least recently used entry
    async fn evict_lru(&self, entries: &mut HashMap<CacheKey, CacheEntry>) {
        if let Some((key_to_remove, _)) = entries
            .iter()
            .min_by_key(|(_, entry)| (entry.access_count, entry.created_at))
        {
            let key_to_remove = key_to_remove.clone();
            entries.remove(&key_to_remove);

            let mut stats = self.stats.write().await;
            stats.evictions += 1;
        }
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get the number of entries in the cache
    pub async fn size(&self) -> usize {
        let entries = self.entries.read().await;
        entries.len()
    }

    /// Get the hit rate (hits / total requests)
    pub async fn hit_rate(&self) -> f64 {
        let stats = self.stats.read().await;
        let total = stats.hits + stats.misses;
        if total == 0 {
            0.0
        } else {
            stats.hits as f64 / total as f64
        }
    }
}

impl Clone for ResponseCache {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            entries: Arc::clone(&self.entries),
            stats: Arc::clone(&self.stats),
        }
    }
}

/// Statistics about cache usage
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// Number of cache hits
    pub hits: u64,
    /// Number of cache misses
    pub misses: u64,
    /// Number of evictions
    pub evictions: u64,
}

impl CacheStats {
    /// Get the total number of requests
    pub fn total_requests(&self) -> u64 {
        self.hits + self.misses
    }

    /// Get the hit rate
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{Role, Usage};

    fn create_message(content: &str) -> Message {
        Message::user(content)
    }

    fn create_response(content: &str) -> GenerateResponse {
        GenerateResponse {
            content: content.to_string(),
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }),
            model: "test-model".to_string(),
            finish_reason: Some("stop".to_string()),
        }
    }

    #[test]
    fn test_cache_key_same_for_identical_requests() {
        let messages = vec![create_message("Hello")];
        let options = Some(GenerateOptions {
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            stop: None,
        });

        let key1 = CacheKey::from_request(&messages, "model", &options);
        let key2 = CacheKey::from_request(&messages, "model", &options);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_different_for_different_messages() {
        let messages1 = vec![create_message("Hello")];
        let messages2 = vec![create_message("Hi")];

        let key1 = CacheKey::from_request(&messages1, "model", &None);
        let key2 = CacheKey::from_request(&messages2, "model", &None);

        assert_ne!(key1, key2);
    }

    #[tokio::test]
    async fn test_cache_hit() {
        let cache = ResponseCache::new(CacheConfig::default());
        let key = CacheKey::from_request(&vec![create_message("test")], "model", &None);
        let response = create_response("cached response");

        cache.put(key.clone(), response.clone()).await;

        let cached = cache.get(&key).await;
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().content, "cached response");

        let stats = cache.stats().await;
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 0);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = ResponseCache::new(CacheConfig::default());
        let key = CacheKey::from_request(&vec![create_message("test")], "model", &None);

        let cached = cache.get(&key).await;
        assert!(cached.is_none());

        let stats = cache.stats().await;
        assert_eq!(stats.hits, 0);
        assert_eq!(stats.misses, 1);
    }

    #[tokio::test]
    async fn test_cache_expiration() {
        let config = CacheConfig {
            enabled: true,
            ttl: Duration::from_millis(100),
            max_entries: 10,
        };
        let cache = ResponseCache::new(config);
        let key = CacheKey::from_request(&vec![create_message("test")], "model", &None);
        let response = create_response("cached response");

        cache.put(key.clone(), response).await;

        // Should be cached immediately
        assert!(cache.get(&key).await.is_some());

        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should be expired
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_cache_eviction() {
        let config = CacheConfig {
            enabled: true,
            ttl: Duration::from_secs(3600),
            max_entries: 2,
        };
        let cache = ResponseCache::new(config);

        // Add 3 entries (should evict the least used one)
        for i in 0..3 {
            let key = CacheKey::from_request(
                &vec![create_message(&format!("test{}", i))],
                "model",
                &None,
            );
            cache.put(key, create_response(&format!("response{}", i))).await;
        }

        // Cache should have at most 2 entries
        assert!(cache.size().await <= 2);

        let stats = cache.stats().await;
        assert!(stats.evictions > 0);
    }

    #[tokio::test]
    async fn test_cache_disabled() {
        let cache = ResponseCache::new(CacheConfig::disabled());
        let key = CacheKey::from_request(&vec![create_message("test")], "model", &None);
        let response = create_response("cached response");

        cache.put(key.clone(), response).await;

        // Should not cache when disabled
        assert!(cache.get(&key).await.is_none());
    }

    #[tokio::test]
    async fn test_hit_rate() {
        let cache = ResponseCache::new(CacheConfig::default());
        let key = CacheKey::from_request(&vec![create_message("test")], "model", &None);
        let response = create_response("cached response");

        cache.put(key.clone(), response).await;

        // 1 hit
        cache.get(&key).await;
        // 1 miss
        let other_key = CacheKey::from_request(&vec![create_message("other")], "model", &None);
        cache.get(&other_key).await;

        let hit_rate = cache.hit_rate().await;
        assert!((hit_rate - 0.5).abs() < 0.01); // Should be 50%
    }
}
