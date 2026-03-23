use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const CACHE_TTL_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTeam {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TeamCache {
    pub fetched_at: DateTime<Utc>,
    pub teams: Vec<CachedTeam>,
}

pub fn cache_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("jr")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .join(".cache")
            .join("jr")
    }
}

pub fn read_team_cache() -> Result<Option<TeamCache>> {
    let path = cache_dir().join("teams.json");
    if !path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&path)?;
    let cache: TeamCache = serde_json::from_str(&content)?;

    let age = Utc::now() - cache.fetched_at;
    if age.num_days() >= CACHE_TTL_DAYS {
        return Ok(None);
    }

    Ok(Some(cache))
}

pub fn write_team_cache(teams: &[CachedTeam]) -> Result<()> {
    let dir = cache_dir();
    std::fs::create_dir_all(&dir)?;

    let cache = TeamCache {
        fetched_at: Utc::now(),
        teams: teams.to_vec(),
    };

    let content = serde_json::to_string_pretty(&cache)?;
    std::fs::write(dir.join("teams.json"), content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    fn with_temp_cache<F: FnOnce()>(f: F) {
        let _guard = ENV_MUTEX.lock().unwrap();
        let dir = TempDir::new().unwrap();
        // SAFETY: test holds ENV_MUTEX, so no concurrent env access.
        unsafe { std::env::set_var("XDG_CACHE_HOME", dir.path()) };
        f();
        unsafe { std::env::remove_var("XDG_CACHE_HOME") };
    }

    #[test]
    fn read_missing_cache_returns_none() {
        with_temp_cache(|| {
            let result = read_team_cache().unwrap();
            assert!(result.is_none());
        });
    }

    #[test]
    fn write_then_read_returns_data() {
        with_temp_cache(|| {
            let teams = vec![
                CachedTeam {
                    id: "uuid-1".into(),
                    name: "Alpha".into(),
                },
                CachedTeam {
                    id: "uuid-2".into(),
                    name: "Beta".into(),
                },
            ];
            write_team_cache(&teams).unwrap();

            let cache = read_team_cache().unwrap().expect("cache should exist");
            assert_eq!(cache.teams.len(), 2);
            assert_eq!(cache.teams[0].name, "Alpha");
            assert_eq!(cache.teams[1].name, "Beta");
        });
    }

    #[test]
    fn expired_cache_returns_none() {
        with_temp_cache(|| {
            let expired = TeamCache {
                fetched_at: Utc::now() - chrono::Duration::days(8),
                teams: vec![CachedTeam {
                    id: "uuid-1".into(),
                    name: "Old".into(),
                }],
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&expired).unwrap();
            std::fs::write(dir.join("teams.json"), content).unwrap();

            let result = read_team_cache().unwrap();
            assert!(result.is_none(), "expired cache should return None");
        });
    }

    #[test]
    fn valid_cache_within_ttl() {
        with_temp_cache(|| {
            let recent = TeamCache {
                fetched_at: Utc::now() - chrono::Duration::days(3),
                teams: vec![CachedTeam {
                    id: "uuid-1".into(),
                    name: "Recent".into(),
                }],
            };
            let dir = cache_dir();
            std::fs::create_dir_all(&dir).unwrap();
            let content = serde_json::to_string_pretty(&recent).unwrap();
            std::fs::write(dir.join("teams.json"), content).unwrap();

            let cache = read_team_cache().unwrap().expect("cache should be valid");
            assert_eq!(cache.teams.len(), 1);
            assert_eq!(cache.teams[0].name, "Recent");
        });
    }
}
