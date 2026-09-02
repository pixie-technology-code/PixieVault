pub mod biometrics;
pub mod protector;
pub mod session;
pub mod vault_crypto;
pub mod windows_hello;

pub use biometrics::{BiometricAuth, BiometricCapabilities};
pub use protector::{
    get_platform_protector, MockPlatformProtector, PlatformKeyProtector, ProtectorCapabilities,
    ProtectorEntry,
};
pub use session::{AuthStatus, VaultSession};
pub use vault_crypto::{
    CryptoError, EncryptedPayload, MasterKey, VaultCrypto, DEFAULT_UNSECURED_PASSPHRASE,
    LEGACY_GLOBAL_SALT,
};
pub use windows_hello::WindowsHelloProtector;

