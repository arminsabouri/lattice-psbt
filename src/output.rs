use std::collections::BTreeMap;

use crate::{Join, JoinError};
use bitcoin::{
    ScriptBuf, TapLeafHash, XOnlyPublicKey, bip32::KeySource, secp256k1, taproot::TapTree,
};
use psbt_v2::raw;

/// All PSBT output fields except value and script_pubkey.
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct VoutData {
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

impl Join for VoutData {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
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

/// A PSBT output whose value and script_pubkey may not yet be known.
#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct PartialVout {
    pub value: Option<bitcoin::Amount>,
    pub script_pubkey: Option<bitcoin::ScriptBuf>,
    pub data: VoutData,
}

impl std::ops::Deref for PartialVout {
    type Target = VoutData;
    fn deref(&self) -> &VoutData {
        &self.data
    }
}

impl std::ops::DerefMut for PartialVout {
    fn deref_mut(&mut self) -> &mut VoutData {
        &mut self.data
    }
}

impl Join for PartialVout {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            value: self.value.join(&other.value)?,
            script_pubkey: self.script_pubkey.join(&other.script_pubkey)?,
            data: self.data.join(&other.data)?,
        })
    }
}

/// A PSBT output with a known value and script_pubkey.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Vout {
    pub value: bitcoin::Amount,
    pub script_pubkey: bitcoin::ScriptBuf,
    pub data: VoutData,
}

impl std::ops::Deref for Vout {
    type Target = VoutData;
    fn deref(&self) -> &VoutData {
        &self.data
    }
}

impl std::ops::DerefMut for Vout {
    fn deref_mut(&mut self) -> &mut VoutData {
        &mut self.data
    }
}

impl Vout {
    pub fn from_output(output: &bitcoin::transaction::TxOut) -> Self {
        Self {
            value: output.value,
            script_pubkey: output.script_pubkey.clone(),
            data: VoutData::default(),
        }
    }

    pub fn with_value(mut self, value: bitcoin::Amount) -> Self {
        self.value = value;
        self
    }

    pub fn with_script_pubkey(mut self, script_pubkey: bitcoin::ScriptBuf) -> Self {
        self.script_pubkey = script_pubkey;
        self
    }
}

impl Join for Vout {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        if self.value != other.value || self.script_pubkey != other.script_pubkey {
            return Err(JoinError::ScalarDisagree);
        }
        Ok(Self {
            value: self.value,
            script_pubkey: self.script_pubkey.clone(),
            data: self.data.join(&other.data)?,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VoutConversionError {
    #[error("Missing output value (amount)")]
    MissingValue,
    #[error("Missing output script pubkey")]
    MissingScriptPubkey,
}

impl TryFrom<PartialVout> for Vout {
    type Error = VoutConversionError;
    fn try_from(partial: PartialVout) -> Result<Self, Self::Error> {
        Ok(Self {
            value: partial.value.ok_or(VoutConversionError::MissingValue)?,
            script_pubkey: partial
                .script_pubkey
                .ok_or(VoutConversionError::MissingScriptPubkey)?,
            data: partial.data,
        })
    }
}

impl From<Vout> for PartialVout {
    fn from(vout: Vout) -> Self {
        Self {
            value: Some(vout.value),
            script_pubkey: Some(vout.script_pubkey),
            data: vout.data,
        }
    }
}
