use super::vault_crypto::MasterKey;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AuthStatus {
    Locked,
    Unlocked,
}

pub struct VaultSession {
    pub status: AuthStatus,
    pub active_key: Option<MasterKey>,
    pub unlocked_at: Option<DateTime<Utc>>,
    pub last_activity: Option<DateTime<Utc>>,
    pub auto_lock_timeout_secs: u64,
    pub active_app_id: Option<String>,
    pub is_initial_unconfigured_passphrase: bool,
}

impl Default for VaultSession {
    fn default() -> Self {
        Self {
            status: AuthStatus::Locked,
            active_key: None,
            unlocked_at: None,
            last_activity: None,
            auto_lock_timeout_secs: 900, // 15 minutes default
            active_app_id: None,
            is_initial_unconfigured_passphrase: false,
        }
    }
}

impl VaultSession {
    pub fn unlock(&mut self, key: MasterKey, is_initial_unconfigured: bool) {
        let now = Utc::now();
        self.active_key = Some(key);
        self.status = AuthStatus::Unlocked;
        self.unlocked_at = Some(now);
        self.last_activity = Some(now);
        self.is_initial_unconfigured_passphrase = is_initial_unconfigured;
    }

    pub fn lock(&mut self) {
        self.active_key = None; // MasterKey implements ZeroizeOnDrop
        self.status = AuthStatus::Locked;
        self.unlocked_at = None;
        self.last_activity = None;
        self.is_initial_unconfigured_passphrase = false;
    }

    pub fn record_activity(&mut self) {
        if self.is_unlocked() {
            self.last_activity = Some(Utc::now());
        }
    }

    pub fn is_unlocked(&self) -> bool {
        self.status == AuthStatus::Unlocked && self.active_key.is_some()
    }

    pub fn is_auto_lock_expired(&self) -> bool {
        if !self.is_unlocked() || self.auto_lock_timeout_secs == 0 {
            return false;
        }
        if let Some(last) = self.last_activity {
            let elapsed = Utc::now() - last;
            elapsed > Duration::seconds(self.auto_lock_timeout_secs as i64)
        } else {
            false
        }
    }

    pub fn set_auto_lock_timeout_secs(&mut self, secs: u64) {
        self.auto_lock_timeout_secs = secs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_lifecycle_and_auto_lock() {
        let mut session = VaultSession::default();
        assert!(!session.is_unlocked());
        assert!(!session.is_auto_lock_expired());

        let key = MasterKey([1u8; 32]);
        session.unlock(key, false);
        assert!(session.is_unlocked());
        assert!(!session.is_auto_lock_expired());

        // Simulate expired activity
        session.last_activity = Some(Utc::now() - Duration::seconds(1000));
        assert!(session.is_auto_lock_expired());

        // Refresh activity
        session.record_activity();
        assert!(!session.is_auto_lock_expired());

        session.lock();
        assert!(!session.is_unlocked());
        assert!(session.active_key.is_none());
    }
}

