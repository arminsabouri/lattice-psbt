use std::collections::BTreeMap;

use bitcoin::bip32::KeySource;
use psbt_v2::raw;

use crate::{JoinError, PartialJoin};

/// Global PSBT field
#[derive(Default, Debug, Clone)]
pub struct Global {
    pub tx_version: Option<bitcoin::transaction::Version>,
    pub fallback_lock_time: Option<bitcoin::locktime::absolute::LockTime>,
    pub xpubs: BTreeMap<bitcoin::bip32::Xpub, KeySource>,
    pub proprietaries: BTreeMap<raw::ProprietaryKey, Vec<u8>>,
    pub unknowns: BTreeMap<raw::Key, Vec<u8>>,
}

impl PartialJoin for Global {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            tx_version: self.tx_version.join(&other.tx_version)?,
            fallback_lock_time: self.fallback_lock_time.join(&other.fallback_lock_time)?,
            xpubs: self.xpubs.join(&other.xpubs)?,
            proprietaries: self.proprietaries.join(&other.proprietaries)?,
            unknowns: self.unknowns.join(&other.unknowns)?,
        })
    }
}
