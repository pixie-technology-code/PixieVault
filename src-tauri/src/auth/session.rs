use super::vault_crypto::MasterKey;
use chrono::{DateTime, Utc};
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
    pub auto_lock_timeout_secs: u64,
    pub active_app_id: Option<String>,
}

impl Default for VaultSession {
    fn default() -> Self {
        Self {
            status: AuthStatus::Locked,
            active_key: None,
            unlocked_at: None,
            auto_lock_timeout_secs: 900, // 15 minutes default
            active_app_id: None,
        }
    }
}

impl VaultSession {
    pub fn unlock(&mut self, key: MasterKey) {
        self.active_key = Some(key);
        self.status = AuthStatus::Unlocked;
        self.unlocked_at = Some(Utc::now());
    }

    pub fn lock(&mut self) {
        self.active_key = None; // MasterKey implements ZeroizeOnDrop
        self.status = AuthStatus::Locked;
        self.unlocked_at = None;
    }

    pub fn is_unlocked(&self) -> bool {
        self.status == AuthStatus::Unlocked && self.active_key.is_some()
    }
}
