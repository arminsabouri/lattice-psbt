use std::collections::HashMap;

use bitcoin::ScriptBuf;

pub use psbt_v2::v2::Output;

use crate::partial_join::PartialJoin;
use crate::values::ValueError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSet(HashMap<ScriptBuf, Output>); // FIXME Vec<u8> not ScriptBuf

impl OutputSet {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn insert(&mut self, output: &Output) -> Result<(), ValueError> {
        use std::collections::hash_map::Entry::*;

        match self.0.entry(output.script_pubkey.clone()) {
            // FIXME don't key by script_pubkey but by PSBT_OUT_UNIQUE_ID
            Occupied(mut entry) => {
                entry.insert(entry.get().join(&output)?);
            }
            Vacant(entry) => {
                entry.insert(output.clone());
            }
        };

        Ok(())
    }
}
impl PartialJoin for OutputSet {
    type Error = ValueError;

    fn join(&self, other: &Self) -> Result<Self, ValueError> {
        let mut new = self.clone();
        for output in other.0.values() {
            new.insert(output)?
        }
        Ok(new)
    }
}

impl PartialJoin for Output {
    type Error = ValueError;
    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        Ok(Self {
            amount: self.amount.join(&other.amount)?,
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
