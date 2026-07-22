//! TTL付き汎用キャッシュ(APIレスポンスの保存・読込・有効期限判定)。
//!
//! 任意の `Serialize + DeserializeOwned` なデータをキー付きでJSONファイルに保存する。
//! 有効期限判定は純粋関数 `is_expired` に分離し、現在時刻は呼び出し側から渡す設計に
//! することで単体テストを可能にしている。

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// キャッシュファイルに保存するエントリ。保存時刻(UNIX秒)とデータ本体を持つ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    /// 保存時刻(UNIX秒)
    pub saved_at_unix: u64,
    /// データ本体
    pub data: T,
}

/// 保存時刻 `saved_at_unix` のエントリが、現在時刻 `now_unix` において
/// TTL(`ttl_secs` 秒)を超過しているかを判定する純粋関数。
///
/// 経過時間がTTLちょうどの場合は期限内とみなす。
/// 保存時刻が未来(時計の巻き戻り等)の場合は期限内とみなす。
pub fn is_expired(saved_at_unix: u64, now_unix: u64, ttl_secs: u64) -> bool {
    now_unix.saturating_sub(saved_at_unix) > ttl_secs
}

/// 現在時刻をUNIX秒で返す(本番コード用ヘルパー)。
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// TTL付きファイルキャッシュ。キーごとに1つのJSONファイルとして保存する。
#[derive(Debug, Clone)]
pub struct Cache {
    dir: PathBuf,
    ttl: Duration,
}

impl Cache {
    /// 保存先ディレクトリとTTLを指定してキャッシュを作成する。
    pub fn new(dir: PathBuf, ttl: Duration) -> Self {
        Self { dir, ttl }
    }

    /// キーに対応するキャッシュファイルのパスを返す。
    /// キーはファイル名として安全な文字(英数字・`-`・`_`・`.`)以外を `_` に置換する。
    fn path_for(&self, key: &str) -> PathBuf {
        let safe: String = key
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.dir.join(format!("{safe}.json"))
    }

    /// データを現在時刻付きでキャッシュに保存する(本番用。時刻はSystemTimeから取得)。
    pub fn store<T: Serialize>(&self, key: &str, data: &T) -> Result<(), AppError> {
        self.store_at(key, data, now_unix())
    }

    /// 保存時刻を明示指定してデータをキャッシュに保存する(テスト可能な本体)。
    pub fn store_at<T: Serialize>(
        &self,
        key: &str,
        data: &T,
        saved_at_unix: u64,
    ) -> Result<(), AppError> {
        fs::create_dir_all(&self.dir).map_err(|e| AppError::Io(e.to_string()))?;
        let entry = CacheEntry {
            saved_at_unix,
            data,
        };
        let json = serde_json::to_string(&entry).map_err(|e| AppError::CacheParse(e.to_string()))?;
        fs::write(self.path_for(key), json).map_err(|e| AppError::Io(e.to_string()))
    }

    /// キーに対応するエントリを読み込む。ファイルなし・パース失敗時は `None`。
    fn load_entry<T: DeserializeOwned>(&self, key: &str) -> Option<CacheEntry<T>> {
        let content = fs::read_to_string(self.path_for(key)).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// 期限内のキャッシュのみを返す。期限切れ・ファイルなし・破損時は `None`。
    pub fn load_fresh<T: DeserializeOwned>(&self, key: &str, now_unix: u64) -> Option<T> {
        let entry = self.load_entry::<T>(key)?;
        if is_expired(entry.saved_at_unix, now_unix, self.ttl.as_secs()) {
            None
        } else {
            Some(entry.data)
        }
    }

    /// 期限切れでもキャッシュがあれば返す(ネットワークエラー時のフォールバック用)。
    pub fn load_any<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.load_entry::<T>(key).map(|entry| entry.data)
    }
}

/// OS標準のキャッシュディレクトリ(`directories` による解決)を返す。
pub fn default_cache_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "weather").map(|dirs| dirs.cache_dir().to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    const TTL_SECS: u64 = 600; // 10分

    /// テスト用の一意な一時ディレクトリパスを返す(ディレクトリ自体は作らない。
    /// `store` が作成することを確認するため)。
    fn make_temp_dir(label: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "weather_cache_test_{}_{}_{}",
            std::process::id(),
            label,
            n
        ))
    }

    fn make_cache(label: &str) -> (Cache, PathBuf) {
        let dir = make_temp_dir(label);
        (Cache::new(dir.clone(), Duration::from_secs(TTL_SECS)), dir)
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct SampleData {
        name: String,
        value: i32,
    }

    fn sample() -> SampleData {
        SampleData {
            name: "東京の予報".to_string(),
            value: 42,
        }
    }

    // --- is_expired(純粋関数) ---

    #[test]
    fn ttl内なら期限切れではない() {
        assert!(!is_expired(1000, 1000 + TTL_SECS - 1, TTL_SECS));
    }

    #[test]
    fn 経過時間がttlちょうどなら期限内とみなす() {
        assert!(!is_expired(1000, 1000 + TTL_SECS, TTL_SECS));
    }

    #[test]
    fn ttlを1秒でも超えたら期限切れ() {
        assert!(is_expired(1000, 1000 + TTL_SECS + 1, TTL_SECS));
    }

    #[test]
    fn 保存時刻が未来でも期限内とみなす() {
        // 時計の巻き戻り等でsaved_at > nowになってもパニックせず期限内扱い
        assert!(!is_expired(2000, 1000, TTL_SECS));
    }

    // --- store / load_fresh / load_any ---

    #[test]
    fn 保存したデータを期限内に読み出せる() {
        let (cache, dir) = make_cache("fresh");
        cache.store_at("forecast_130000", &sample(), 1000).expect("保存失敗");
        let loaded: Option<SampleData> = cache.load_fresh("forecast_130000", 1000 + 60);
        assert_eq!(loaded, Some(sample()));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 期限切れのデータはload_freshでは読めない() {
        let (cache, dir) = make_cache("expired");
        cache.store_at("key", &sample(), 1000).expect("保存失敗");
        let loaded: Option<SampleData> = cache.load_fresh("key", 1000 + TTL_SECS + 1);
        assert_eq!(loaded, None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 期限切れでもload_anyなら読み出せる() {
        let (cache, dir) = make_cache("fallback");
        cache.store_at("key", &sample(), 1000).expect("保存失敗");
        let loaded: Option<SampleData> = cache.load_any("key");
        assert_eq!(loaded, Some(sample()));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 存在しないキーはnoneを返す() {
        let (cache, dir) = make_cache("missing");
        let fresh: Option<SampleData> = cache.load_fresh("no_such_key", 1000);
        let any: Option<SampleData> = cache.load_any("no_such_key");
        assert_eq!(fresh, None);
        assert_eq!(any, None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 破損したキャッシュファイルはnoneを返す() {
        let (cache, dir) = make_cache("corrupt");
        fs::create_dir_all(&dir).expect("ディレクトリ作成失敗");
        fs::write(dir.join("broken.json"), "{ not json !").expect("書き込み失敗");
        let fresh: Option<SampleData> = cache.load_fresh("broken", 1000);
        let any: Option<SampleData> = cache.load_any("broken");
        assert_eq!(fresh, None);
        assert_eq!(any, None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn 上書き保存すると新しいデータが読める() {
        let (cache, dir) = make_cache("overwrite");
        cache.store_at("key", &sample(), 1000).expect("保存失敗");
        let updated = SampleData {
            name: "更新後".to_string(),
            value: 7,
        };
        cache.store_at("key", &updated, 2000).expect("保存失敗");
        let loaded: Option<SampleData> = cache.load_fresh("key", 2000);
        assert_eq!(loaded, Some(updated));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ファイル名に使えない文字を含むキーでも保存読込できる() {
        let (cache, dir) = make_cache("sanitize");
        cache.store_at("東京/予報:今日", &sample(), 1000).expect("保存失敗");
        let loaded: Option<SampleData> = cache.load_fresh("東京/予報:今日", 1000);
        assert_eq!(loaded, Some(sample()));
        fs::remove_dir_all(&dir).ok();
    }
}
