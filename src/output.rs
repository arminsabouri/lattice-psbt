use std::collections::BTreeMap;

use crate::{Join, JoinError};
use bitcoin::{
    ScriptBuf, TapLeafHash, XOnlyPublicKey, bip32::KeySource, secp256k1, taproot::TapTree,
};
use psbt_v2::raw;

#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Vout {
    pub value: Option<bitcoin::Amount>,
    pub script_pubkey: Option<bitcoin::ScriptBuf>,
    /// The redeem script for this output.
    pub redeem_script: Option<ScriptBuf>,
    /// The witness script for this output.
    pub witness_script: Option<ScriptBuf>,
    /// A map from public keys needed to spend this output to their
    /// corresponding master key fingerprints and derivation paths.
    pub bip32_derivations: BTreeMap<secp256k1::PublicKey, KeySource>,
    /// The internal pubkey.
    pub tap_internal_key: Option<XOnlyPublicKey>,
    /// Taproot Output tree.
    pub tap_tree: Option<TapTree>,
    pub tap_key_origins: BTreeMap<XOnlyPublicKey, (Vec<TapLeafHash>, KeySource)>,
    /// Proprietary key-value pairs for this output.
    pub proprietaries: BTreeMap<raw::ProprietaryKey, Vec<u8>>,
    /// Unknown key-value pairs for this output.
    pub unknowns: BTreeMap<raw::Key, Vec<u8>>,
}

impl Vout {
    pub fn from_output(output: &bitcoin::transaction::TxOut) -> Self {
        Self {
            value: Some(output.value),
            script_pubkey: Some(output.script_pubkey.clone()),
            ..Default::default()
        }
    }

    pub fn with_value(mut self, value: bitcoin::Amount) -> Self {
        self.value = Some(value);
        self
    }

    pub fn with_script_pubkey(mut self, script_pubkey: bitcoin::ScriptBuf) -> Self {
        self.script_pubkey = Some(script_pubkey);
        self
    }
}

impl Join for Vout {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            value: self.value.join(&other.value)?,
            script_pubkey: self.script_pubkey.join(&other.script_pubkey)?,
            redeem_script: self.redeem_script.join(&other.redeem_script)?,
            witness_script: self.witness_script.join(&other.witness_script)?,
            tap_internal_key: self.tap_internal_key.join(&other.tap_internal_key)?,
            tap_tree: self.tap_tree.join(&other.tap_tree)?,
            bip32_derivations: self.bip32_derivations.join(&other.bip32_derivations)?,
            tap_key_origins: self.tap_key_origins.join(&other.tap_key_origins)?,
            proprietaries: self.proprietaries.join(&other.proprietaries)?,
            unknowns: self.unknowns.join(&other.unknowns)?,
        })
    }
}
