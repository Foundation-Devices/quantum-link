pub const ML_KEM_SUITE_TAG: &[u8] = b"ml-kem-1024";

// ql-wire fixes the protocol to ML-KEM-1024 on the wire, but the host
// platform is free to satisfy QlKem with any backend that produces the same
// serialized sizes.
const ML_KEM_1024_SHARED_SECRET_SIZE: usize = 32;
const ML_KEM_1024_PUBLIC_KEY_SIZE: usize = 1568;
const ML_KEM_1024_PRIVATE_KEY_SIZE: usize = 3168;
const ML_KEM_1024_CIPHERTEXT_SIZE: usize = 1568;

#[derive(Clone, PartialEq, Eq, Hash, ql_codec::Codec)]
#[codec(frozen)]
pub struct SessionKey(pub [u8; SessionKey::SIZE]);

impl std::fmt::Debug for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionKey(<redacted>)")
    }
}

impl SessionKey {
    pub const SIZE: usize = ML_KEM_1024_SHARED_SECRET_SIZE;

    pub const fn as_bytes(&self) -> &[u8; Self::SIZE] {
        &self.0
    }
}

impl AsRef<[u8]> for SessionKey {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Drop for SessionKey {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ql_codec::Codec)]
#[codec(frozen)]
pub struct MlKemPublicKey(Box<[u8; MlKemPublicKey::SIZE]>);

impl MlKemPublicKey {
    pub const SIZE: usize = ML_KEM_1024_PUBLIC_KEY_SIZE;

    pub fn new(data: Box<[u8; Self::SIZE]>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8; Self::SIZE] {
        self.0.as_ref()
    }
}

impl Drop for MlKemPublicKey {
    fn drop(&mut self) {
        self.0.as_mut().fill(0);
    }
}

#[derive(Clone, PartialEq, Eq, Hash, ql_codec::Codec)]
#[codec(frozen)]
pub struct MlKemPrivateKey(Box<[u8; MlKemPrivateKey::SIZE]>);

impl std::fmt::Debug for MlKemPrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MlKemPrivateKey(<redacted>)")
    }
}

impl MlKemPrivateKey {
    pub const SIZE: usize = ML_KEM_1024_PRIVATE_KEY_SIZE;

    pub fn new(data: Box<[u8; Self::SIZE]>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8; Self::SIZE] {
        self.0.as_ref()
    }
}

impl Drop for MlKemPrivateKey {
    fn drop(&mut self) {
        self.0.as_mut().fill(0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ql_codec::Codec)]
#[codec(frozen)]
pub struct MlKemCiphertext(Box<[u8; MlKemCiphertext::SIZE]>);

impl MlKemCiphertext {
    pub const SIZE: usize = ML_KEM_1024_CIPHERTEXT_SIZE;

    pub fn new(data: Box<[u8; Self::SIZE]>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8; Self::SIZE] {
        self.0.as_ref()
    }
}

impl Drop for MlKemCiphertext {
    fn drop(&mut self) {
        self.0.as_mut().fill(0);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MlKemKeyPair {
    pub private: MlKemPrivateKey,
    pub public: MlKemPublicKey,
}
