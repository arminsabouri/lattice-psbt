use bitcoin::OutPoint;

use std::collections::HashMap;

use std::hash::{Hash, Hasher};
use std::ops::Deref;

pub use psbt_v2::v2::Input;

use crate::partial_join::PartialJoin;
use crate::values::ValueError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputSet(HashMap<OutPoint, Input>);

impl Default for InputSet {
    fn default() -> Self {
        InputSet(HashMap::new())
    }
}

impl InputSet {
    pub fn spends_outpoint(&self, outpoint: &OutPoint) -> bool {
        self.0.contains_key(outpoint)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn insert(&mut self, input: &Input) -> Result<(), ValueError> {
        use std::collections::hash_map::Entry::*;

        match self.0.entry(OutPoint {
            txid: input.previous_txid,
            vout: input.spent_output_index,
        }) {
            Occupied(mut entry) => {
                entry.insert(entry.get().join(input)?);
            }
            Vacant(entry) => {
                entry.insert(input.clone());
            }
        };

        Ok(())
    }
}

impl PartialJoin for InputSet {
    type Error = ValueError;

    fn join(&self, other: &Self) -> Result<Self, ValueError> {
        let mut new = self.clone();
        for input in other.0.values() {
            new.insert(input)?
        }
        Ok(new)
    }
}

impl PartialJoin for Input {
    type Error = ValueError;

    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        Ok(Self {
            previous_txid: self.previous_txid.join(&other.previous_txid)?,
            spent_output_index: self.spent_output_index.join(&other.spent_output_index)?,
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
