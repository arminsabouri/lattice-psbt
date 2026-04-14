use std::collections::BTreeMap;

use psbt_v2::{PsbtSighashType, raw};

use crate::{Join, JoinError};

/// All PSBT input fields except the outpoint (previous_output, spent_output_index).
#[derive(Default, Clone, PartialEq, Eq, Hash, Debug)]
pub struct VinData {
    /// The sequence number of this input.
    ///
    /// If omitted, assumed to be the final sequence number ([`Sequence::MAX`]).
    pub sequence: Option<bitcoin::Sequence>,

    /// The minimum Unix timestamp that this input requires to be set as the transaction's lock time.
    pub min_time: Option<bitcoin::absolute::Time>,

    /// The minimum block height that this input requires to be set as the transaction's lock time.
    pub min_height: Option<bitcoin::absolute::Height>,

    /// The non-witness transaction this input spends from. Should only be
    /// `Option::Some` for inputs which spend non-segwit outputs or
    /// if it is unknown whether an input spends a segwit output.
    pub non_witness_utxo: Option<bitcoin::Transaction>,
    /// The transaction output this input spends from. Should only be
    /// `Option::Some` for inputs which spend segwit outputs,
    /// including P2SH embedded ones.
    pub witness_utxo: Option<bitcoin::TxOut>,
    /// A map from public keys to their corresponding signature as would be
    /// pushed to the stack from a scriptSig or witness for a non-taproot inputs.
    pub partial_sigs: BTreeMap<bitcoin::PublicKey, bitcoin::ecdsa::Signature>,
    /// The sighash type to be used for this input. Signatures for this input
    /// must use the sighash type.
    pub sighash_type: Option<PsbtSighashType>,
    /// The redeem script for this input.
    pub redeem_script: Option<bitcoin::ScriptBuf>,
    /// The witness script for this input.
    pub witness_script: Option<bitcoin::ScriptBuf>,
    /// A map from public keys needed to sign this input to their corresponding
    /// master key fingerprints and derivation paths.
    pub bip32_derivations: BTreeMap<bitcoin::secp256k1::PublicKey, bitcoin::bip32::KeySource>,
    /// The finalized, fully-constructed scriptSig with signatures and any other
    /// scripts necessary for this input to pass validation.
    pub final_script_sig: Option<bitcoin::ScriptBuf>,
    /// The finalized, fully-constructed scriptWitness with signatures and any
    /// other scripts necessary for this input to pass validation.
    pub final_script_witness: Option<bitcoin::Witness>,
    /// TODO: Proof of reserves commitment
    /// RIPEMD160 hash to preimage map.
    pub ripemd160_preimages: BTreeMap<bitcoin::hashes::ripemd160::Hash, Vec<u8>>,
    /// SHA256 hash to preimage map.
    pub sha256_preimages: BTreeMap<bitcoin::hashes::sha256::Hash, Vec<u8>>,
    /// HSAH160 hash to preimage map.
    pub hash160_preimages: BTreeMap<bitcoin::hashes::hash160::Hash, Vec<u8>>,
    /// HAS256 hash to preimage map.
    pub hash256_preimages: BTreeMap<bitcoin::hashes::sha256d::Hash, Vec<u8>>,
    /// Serialized taproot signature with sighash type for key spend.
    pub tap_key_sig: Option<bitcoin::taproot::Signature>,
    /// Map of `<xonlypubkey>|<leafhash>` with signature.
    pub tap_script_sigs:
        BTreeMap<(bitcoin::XOnlyPublicKey, bitcoin::TapLeafHash), bitcoin::taproot::Signature>,
    /// Map of Control blocks to Script version pair.
    pub tap_scripts: BTreeMap<
        bitcoin::taproot::ControlBlock,
        (bitcoin::ScriptBuf, bitcoin::taproot::LeafVersion),
    >,
    /// Map of tap root x only keys to origin info and leaf hashes contained in it.
    pub tap_key_origins:
        BTreeMap<bitcoin::XOnlyPublicKey, (Vec<bitcoin::TapLeafHash>, bitcoin::bip32::KeySource)>,
    /// Taproot Internal key.
    pub tap_internal_key: Option<bitcoin::XOnlyPublicKey>,
    /// Taproot Merkle root.
    pub tap_merkle_root: Option<bitcoin::TapNodeHash>,
    /// Proprietary key-value pairs for this input.
    pub proprietaries: BTreeMap<raw::ProprietaryKey, Vec<u8>>,
    /// Unknown key-value pairs for this input.
    pub unknowns: BTreeMap<raw::Key, Vec<u8>>,
}

impl Join for VinData {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            sequence: self.sequence.join(&other.sequence)?,
            min_time: self.min_time.join(&other.min_time)?,
            min_height: self.min_height.join(&other.min_height)?,
            non_witness_utxo: self.non_witness_utxo.join(&other.non_witness_utxo)?,
            witness_utxo: self.witness_utxo.join(&other.witness_utxo)?,
            partial_sigs: self.partial_sigs.join(&other.partial_sigs)?,
            sighash_type: self.sighash_type.join(&other.sighash_type)?,
            redeem_script: self.redeem_script.join(&other.redeem_script)?,
            witness_script: self.witness_script.join(&other.witness_script)?,
            bip32_derivations: self.bip32_derivations.join(&other.bip32_derivations)?,
            final_script_sig: self.final_script_sig.join(&other.final_script_sig)?,
            final_script_witness: self
                .final_script_witness
                .join(&other.final_script_witness)?,
            ripemd160_preimages: self.ripemd160_preimages.join(&other.ripemd160_preimages)?,
            sha256_preimages: self.sha256_preimages.join(&other.sha256_preimages)?,
            hash160_preimages: self.hash160_preimages.join(&other.hash160_preimages)?,
            hash256_preimages: self.hash256_preimages.join(&other.hash256_preimages)?,
            tap_key_sig: self.tap_key_sig.join(&other.tap_key_sig)?,
            tap_script_sigs: self.tap_script_sigs.join(&other.tap_script_sigs)?,
            tap_scripts: self.tap_scripts.join(&other.tap_scripts)?,
            tap_key_origins: self.tap_key_origins.join(&other.tap_key_origins)?,
            tap_internal_key: self.tap_internal_key.join(&other.tap_internal_key)?,
            tap_merkle_root: self.tap_merkle_root.join(&other.tap_merkle_root)?,
            proprietaries: self.proprietaries.join(&other.proprietaries)?,
            unknowns: self.unknowns.join(&other.unknowns)?,
        })
    }
}

/// A PSBT input whose outpoint (previous tx + vout) may not yet be known.
#[derive(Default, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PartialVin {
    /// The txid of the previous transaction output being spent, if known.
    pub previous_output: Option<bitcoin::Txid>,
    /// The index of the output being spent, if known.
    pub spent_output_index: Option<u32>,
    pub data: VinData,
}

impl std::ops::Deref for PartialVin {
    type Target = VinData;
    fn deref(&self) -> &VinData {
        &self.data
    }
}

impl std::ops::DerefMut for PartialVin {
    fn deref_mut(&mut self) -> &mut VinData {
        &mut self.data
    }
}

impl Join for PartialVin {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            previous_output: self.previous_output.join(&other.previous_output)?,
            spent_output_index: self.spent_output_index.join(&other.spent_output_index)?,
            data: self.data.join(&other.data)?,
        })
    }
}

/// A PSBT input with a known outpoint.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Vin {
    /// The txid of the previous transaction output being spent.
    pub previous_output: bitcoin::Txid,
    /// The index of the output being spent.
    pub spent_output_index: u32,
    pub data: VinData,
}

impl std::ops::Deref for Vin {
    type Target = VinData;
    fn deref(&self) -> &VinData {
        &self.data
    }
}

impl std::ops::DerefMut for Vin {
    fn deref_mut(&mut self) -> &mut VinData {
        &mut self.data
    }
}

impl Vin {
    pub fn from_input(input: &bitcoin::transaction::TxIn) -> Self {
        Self {
            previous_output: input.previous_output.txid,
            spent_output_index: input.previous_output.vout,
            data: VinData::default(),
        }
    }
}

impl Join for Vin {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        if self.previous_output != other.previous_output
            || self.spent_output_index != other.spent_output_index
        {
            return Err(JoinError::ScalarDisagree);
        }
        Ok(Self {
            previous_output: self.previous_output,
            spent_output_index: self.spent_output_index,
            data: self.data.join(&other.data)?,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VinConversionError {
    #[error("Missing previous output txid")]
    MissingTxid,
    #[error("Missing spent output index (vout)")]
    MissingVout,
}

impl TryFrom<PartialVin> for Vin {
    type Error = VinConversionError;
    fn try_from(partial: PartialVin) -> Result<Self, Self::Error> {
        Ok(Self {
            previous_output: partial
                .previous_output
                .ok_or(VinConversionError::MissingTxid)?,
            spent_output_index: partial
                .spent_output_index
                .ok_or(VinConversionError::MissingVout)?,
            data: partial.data,
        })
    }
}

impl From<Vin> for PartialVin {
    fn from(vin: Vin) -> Self {
        Self {
            previous_output: Some(vin.previous_output),
            spent_output_index: Some(vin.spent_output_index),
            data: vin.data,
        }
    }
}
