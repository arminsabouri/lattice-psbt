use bitcoin::{TapLeafHash, XOnlyPublicKey, bip32::KeySource};
use psbt_v2::{raw, v2::Psbt};
use rand::{SeedableRng, seq::SliceRandom};
use std::{
    collections::{BTreeMap, HashSet},
    ops::Deref,
};

pub mod global;
pub mod input;
pub mod output;

use global::Global;
use input::Vin;
use output::Vout;

/*
Our goals: Create a monotone datastructure that can take un ordered transaction components can merge or joins them if they are non-conflicting.
Such a datastructure should have eventual consistency. We will model a PSBT as a join semi lattice set.
A lattice set in our context is a poset of partial transaction components, s.t any two postets can have a greatest lower bound.

- Define what is a field lattice: a single truth aka scalar values (nlocktime, nversion, witness for an input, etc...)
- Define what is a set field value: accumulated facts: xpubs, derivations, etc...

Each field/scope (global, each input, each output) can be modeled as a semilattice
The whole PSBT (a semi lattice also) is then the product of those components (also a semi lattice). merge is the componentwise join.

We need to define a each transaction component as either a scalar value that can be unknown or the scalar value itself. And Semi lattice for the sets of values that can accumulate facts monotonically.
Perhaps the best way to do this is to define a Optional field generic over a type (which can be a scalar or a semilattice). And to define a trait for how to compare and join them.
*/

macro_rules! impl_join_field_value {
    ($t:ty) => {
        impl Join for Option<$t> {
            fn join(&self, other: &Self) -> Result<Self, JoinError> {
                match (self, other) {
                    (None, None) => Ok(None),
                    (None, x) | (x, None) => Ok(x.clone()),
                    (Some(a), Some(b)) if a == b => Ok(Some(a.clone())),
                    _ => Err(JoinError::ScalarDisagree),
                }
            }
        }
    };
}

impl_join_field_value!(u32);
impl_join_field_value!(bitcoin::Txid);
impl_join_field_value!(bitcoin::ScriptBuf);
impl_join_field_value!(bitcoin::Witness);
impl_join_field_value!(bitcoin::TxOut);
impl_join_field_value!(bitcoin::Amount);
impl_join_field_value!(bitcoin::Sequence);
impl_join_field_value!(bitcoin::locktime::absolute::LockTime);
impl_join_field_value!(bitcoin::transaction::Version);
impl_join_field_value!(bitcoin::secp256k1::XOnlyPublicKey);
impl_join_field_value!(bitcoin::taproot::TapTree);
impl_join_field_value!(psbt_v2::Version);
impl_join_field_value!(bitcoin::absolute::Time);
impl_join_field_value!(bitcoin::absolute::Height);
impl_join_field_value!(bitcoin::Transaction);
impl_join_field_value!(psbt_v2::PsbtSighashType);
impl_join_field_value!(bitcoin::taproot::Signature);
impl_join_field_value!(bitcoin::TapNodeHash);

// TODO: if there is a key collision and the values are not equal, we should return an error
// TODO: just need one macro for map types
// TODO: remove clones
macro_rules! impl_join_for_hashset {
    ($type:ty) => {
        impl Join for HashSet<$type> {
            fn join(&self, other: &Self) -> Result<Self, JoinError> {
                let mut result = self.clone();
                result.extend(other.iter().cloned());
                Ok(result)
            }
        }
    };
}

impl_join_for_hashset!(Vin);
impl_join_for_hashset!(Vout);

macro_rules! impl_join_for_btreemap {
    ($key:ty, $value:ty) => {
        impl Join for BTreeMap<$key, $value> {
            fn join(&self, other: &Self) -> Result<Self, JoinError> {
                let mut result = self.clone();
                result.extend(other.clone().into_iter());
                Ok(result)
            }
        }
    };
}

impl_join_for_btreemap!(bitcoin::secp256k1::PublicKey, bitcoin::bip32::KeySource);
impl_join_for_btreemap!(raw::ProprietaryKey, Vec<u8>);
impl_join_for_btreemap!(raw::Key, Vec<u8>);
impl_join_for_btreemap!(XOnlyPublicKey, (Vec<TapLeafHash>, KeySource));
impl_join_for_btreemap!(bitcoin::bip32::Xpub, KeySource);

impl_join_for_btreemap!(bitcoin::PublicKey, bitcoin::ecdsa::Signature);
impl_join_for_btreemap!(bitcoin::hashes::sha256::Hash, Vec<u8>);
impl_join_for_btreemap!(bitcoin::hashes::ripemd160::Hash, Vec<u8>);
impl_join_for_btreemap!(bitcoin::hashes::hash160::Hash, Vec<u8>);
impl_join_for_btreemap!(bitcoin::hashes::sha256d::Hash, Vec<u8>);
impl_join_for_btreemap!(
    (bitcoin::XOnlyPublicKey, bitcoin::TapLeafHash),
    bitcoin::taproot::Signature
);
impl_join_for_btreemap!(
    bitcoin::taproot::ControlBlock,
    (bitcoin::ScriptBuf, bitcoin::taproot::LeafVersion)
);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JoinError {
    #[error("Scalar disagree")]
    ScalarDisagree,
    #[error("Structural mismatch / key collision")]
    StructuralMismatch,
}

/// Trait for PSBT fragments that can be joined.
pub trait Join {
    fn join(&self, other: &Self) -> Result<Self, JoinError>
    where
        Self: Sized;
}

pub trait TypeState {}

pub struct Transaction<State: TypeState> {
    state: State,
}

impl<State: TypeState> Transaction<State> {
    pub fn new() -> Transaction<UnOrderedInputs> {
        Transaction {
            state: UnOrderedInputs::default(),
        }
    }
}

impl<State: TypeState> Deref for Transaction<State> {
    type Target = State;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

#[derive(Default, Debug)]
pub struct UnOrderedInputs {
    inputs: HashSet<Vin>,
    outputs: HashSet<Vout>,
    global: Global,
}

impl TypeState for UnOrderedInputs {}

impl Join for UnOrderedInputs {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            inputs: self.inputs.join(&other.inputs)?,
            outputs: self.outputs.join(&other.outputs)?,
            global: self.global.join(&other.global)?,
        })
    }
}

impl Transaction<UnOrderedInputs> {
    pub fn apply_bip69_ordering(self) -> Transaction<OrderedInputs> {
        let mut inputs = self.state.inputs.into_iter().collect::<Vec<_>>();
        inputs.sort_by_key(|input| (input.previous_output, input.spent_output_index));
        Transaction {
            state: OrderedInputs {
                inputs,
                outputs: self.state.outputs.clone(),
                global: self.state.global.clone(),
            },
        }
    }

    pub fn apply_ordering_with_salt(self, salt: &[u8; 32]) -> Transaction<OrderedInputs> {
        let mut rng = rand_chacha::ChaCha20Rng::from_seed(*salt);
        let mut inputs = self.state.inputs.into_iter().collect::<Vec<_>>();
        inputs.shuffle(&mut rng);

        Transaction {
            state: OrderedInputs {
                inputs,
                outputs: self.state.outputs.clone(),
                global: self.state.global.clone(),
            },
        }
    }
}

#[derive(Default, Debug)]
pub struct OrderedInputs {
    inputs: Vec<Vin>,
    outputs: HashSet<Vout>,
    global: Global,
}

impl TypeState for OrderedInputs {}

impl Join for OrderedInputs {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            inputs: self.inputs.clone(),
            outputs: self.outputs.join(&other.outputs)?,
            global: self.global.join(&other.global)?,
        })
    }
}

impl Transaction<OrderedInputs> {
    pub fn apply_bip69_ordering(self) -> Transaction<OrderedOutputs> {
        let mut outputs = self.state.outputs.into_iter().collect::<Vec<_>>();
        outputs.sort_by_key(|output| (output.value, output.script_pubkey.clone()));
        Transaction {
            state: OrderedOutputs {
                inputs: self.state.inputs.clone(),
                outputs,
                global: self.state.global.clone(),
            },
        }
    }

    pub fn apply_ordering_with_salt(self, salt: &[u8; 32]) -> Transaction<OrderedOutputs> {
        let mut rng = rand_chacha::ChaCha20Rng::from_seed(*salt);
        let mut outputs = self.state.outputs.into_iter().collect::<Vec<_>>();
        outputs.shuffle(&mut rng);

        Transaction {
            state: OrderedOutputs {
                inputs: self.state.inputs.clone(),
                outputs,
                global: self.state.global.clone(),
            },
        }
    }
}

#[derive(Default, Debug)]
pub struct OrderedOutputs {
    inputs: Vec<Vin>,
    outputs: Vec<Vout>,
    global: Global,
}

impl TypeState for OrderedOutputs {}

impl Join for OrderedOutputs {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            global: self.global.join(&other.global)?,
        })
    }
}

impl Transaction<OrderedOutputs> {
    pub fn finalize(self) -> Transaction<WithGlobal> {
        Transaction {
            state: WithGlobal {
                inputs: self.state.inputs.clone(),
                outputs: self.state.outputs.clone(),
                global: self.state.global.clone(),
            },
        }
    }
}

#[derive(Default, Debug)]
pub struct WithGlobal {
    inputs: Vec<Vin>,
    outputs: Vec<Vout>,
    global: Global,
}

impl TypeState for WithGlobal {}

#[derive(Debug, thiserror::Error)]
pub enum PsbtConversionError {
    #[error("Missing outpoint txid for index {0}")]
    MissingOutpointTxid(usize),
    #[error("Missing outpoint vout for index {0}")]
    MissingOutpointVout(usize),
    #[error("Missing value for output {0}")]
    MissingValue(usize),
    #[error("Missing script pubkey for output {0}")]
    MissingScriptPubkey(usize),
}

impl TryFrom<Transaction<WithGlobal>> for Psbt {
    type Error = PsbtConversionError;
    fn try_from(psbt: Transaction<WithGlobal>) -> Result<Self, Self::Error> {
        let psbt = psbt.state;
        let tx = psbt_v2::v2::Psbt {
            global: psbt_v2::v2::Global {
                version: psbt_v2::Version::TWO,
                // TODO: is this the right default?
                tx_version: psbt
                    .global
                    .tx_version
                    .unwrap_or(bitcoin::transaction::Version::TWO),
                fallback_lock_time: psbt.global.fallback_lock_time,
                tx_modifiable_flags: 0u8,
                input_count: psbt.inputs.len(),
                output_count: psbt.outputs.len(),
                xpubs: psbt.global.xpubs,
                proprietaries: psbt.global.proprietaries,
                unknowns: psbt.global.unknowns,
            },
            inputs: psbt
                .inputs
                .into_iter()
                .enumerate()
                .map(|(i, input)| {
                    Ok::<psbt_v2::v2::Input, PsbtConversionError>(psbt_v2::v2::Input {
                        previous_txid: input
                            .previous_output
                            .ok_or(PsbtConversionError::MissingOutpointTxid(i))?,
                        spent_output_index: input
                            .spent_output_index
                            .ok_or(PsbtConversionError::MissingOutpointVout(i))?,
                        sequence: input.sequence,
                        witness_utxo: input.witness_utxo,
                        final_script_sig: input.final_script_sig,
                        final_script_witness: input.final_script_witness,
                        min_time: None,
                        min_height: None,
                        non_witness_utxo: None,
                        partial_sigs: BTreeMap::new(),
                        sighash_type: None,
                        redeem_script: None,
                        witness_script: None,
                        bip32_derivations: BTreeMap::new(),
                        ripemd160_preimages: BTreeMap::new(),
                        sha256_preimages: BTreeMap::new(),
                        hash160_preimages: BTreeMap::new(),
                        hash256_preimages: BTreeMap::new(),
                        tap_key_sig: None,
                        tap_script_sigs: BTreeMap::new(),
                        tap_scripts: BTreeMap::new(),
                        tap_key_origins: BTreeMap::new(),
                        tap_internal_key: None,
                        tap_merkle_root: None,
                        proprietaries: BTreeMap::new(),
                        unknowns: BTreeMap::new(),
                    })
                })
                .collect::<Result<Vec<_>, PsbtConversionError>>()?,
            outputs: psbt
                .outputs
                .into_iter()
                .enumerate()
                .map(|(i, output)| {
                    Ok::<psbt_v2::v2::Output, PsbtConversionError>(psbt_v2::v2::Output {
                        amount: output.value.ok_or(PsbtConversionError::MissingValue(i))?,
                        script_pubkey: output
                            .script_pubkey
                            .ok_or(PsbtConversionError::MissingScriptPubkey(i))?,
                        redeem_script: output.redeem_script,
                        witness_script: output.witness_script,
                        bip32_derivations: output.bip32_derivations,
                        tap_internal_key: output.tap_internal_key,
                        tap_tree: output.tap_tree,
                        tap_key_origins: output.tap_key_origins,
                        proprietaries: output.proprietaries,
                        unknowns: output.unknowns,
                    })
                })
                .collect::<Result<Vec<_>, PsbtConversionError>>()?,
        };

        Ok(tx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_flow() {
        let mut tx = Transaction::<UnOrderedInputs>::new();
        let my_vin = Vin::from_input(&bitcoin::transaction::TxIn::default());
        tx.state.inputs.insert(my_vin.clone());

        let mut tx = tx.apply_bip69_ordering();
        let my_vout = Vout::from_output(&bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(1000),
            script_pubkey: bitcoin::ScriptBuf::new(),
        });
        tx.state.outputs.insert(my_vout.clone());
        let tx = tx.apply_bip69_ordering();
        let tx = tx.finalize();
        let psbt = Psbt::try_from(tx).unwrap();
        assert_eq!(psbt.global.tx_version, bitcoin::transaction::Version::TWO);
        assert_eq!(psbt.global.input_count, 1);
        assert_eq!(psbt.global.output_count, 1);
        assert_eq!(psbt.global.xpubs, BTreeMap::new());
        assert_eq!(psbt.global.proprietaries, BTreeMap::new());
        assert_eq!(psbt.global.unknowns, BTreeMap::new());
        assert_eq!(psbt.global.fallback_lock_time, None);
        assert_eq!(psbt.global.tx_modifiable_flags, 0);
        assert_eq!(psbt.global.version, psbt_v2::Version::TWO);

        assert_eq!(
            psbt.inputs[0].previous_txid,
            my_vin.previous_output.unwrap()
        );
        assert_eq!(
            psbt.inputs[0].spent_output_index,
            my_vin.spent_output_index.unwrap()
        );
        // TODO: more input assertions
        assert_eq!(psbt.outputs[0].amount, my_vout.value.unwrap());
        assert_eq!(
            psbt.outputs[0].script_pubkey,
            my_vout.script_pubkey.unwrap()
        );
        assert_eq!(psbt.outputs[0].redeem_script, None);
        assert_eq!(psbt.outputs[0].witness_script, None);
        assert_eq!(psbt.outputs[0].bip32_derivations, BTreeMap::new());
        assert_eq!(psbt.outputs[0].tap_internal_key, None);
        assert_eq!(psbt.outputs[0].tap_tree, None);
        assert_eq!(psbt.outputs[0].tap_key_origins, BTreeMap::new());
        assert_eq!(psbt.outputs[0].proprietaries, BTreeMap::new());
        assert_eq!(psbt.outputs[0].unknowns, BTreeMap::new());
    }

    #[test]
    fn test_join_outputs() {
        let output_amount = bitcoin::Amount::from_sat(1000);
        let output_script_pubkey = bitcoin::ScriptBuf::new();

        let p1 = Vout {
            value: Some(output_amount),
            ..Default::default()
        };
        let p1_again = Vout {
            value: Some(output_amount),
            ..Default::default()
        };
        // Attempting to join p1 and p1_again NOT fail and result in a Vout with value 1000
        let p1_joined = p1.join(&p1_again).unwrap();
        assert_eq!(p1_joined.value, Some(output_amount));

        let p1_with_different_value = Vout {
            value: Some(bitcoin::Amount::from_sat(2000)),
            ..Default::default()
        };
        let p1_joined_with_different_value = p1.join(&p1_with_different_value).err();
        assert_eq!(
            p1_joined_with_different_value,
            Some(JoinError::ScalarDisagree)
        );

        let p1_with_different_script_pubkey = Vout {
            script_pubkey: Some(bitcoin::ScriptBuf::new()),
            ..Default::default()
        };

        let p1_joined_with_different_script_pubkey =
            p1.join(&p1_with_different_script_pubkey).unwrap();
        assert_eq!(
            p1_joined_with_different_script_pubkey.script_pubkey,
            Some(output_script_pubkey)
        );
        assert_eq!(p1_joined.value, Some(output_amount));
    }
}
