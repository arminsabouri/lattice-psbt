use bitcoin::{
    ScriptBuf, TapLeafHash, XOnlyPublicKey, bip32::KeySource, secp256k1, taproot::TapTree,
};
use psbt_v2::{raw, v2::Psbt};
use std::collections::{BTreeMap, HashSet};

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

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JoinError {
    #[error("Scalar disagree")]
    ScalarDisagree,
    #[error("Structural mismatch / key collision")]
    StructuralMismatch,
}

pub trait Join {
    fn join(&self, other: &Self) -> Result<Self, JoinError>
    where
        Self: Sized;
}

#[derive(Default, Debug)]
pub struct Global {
    pub tx_version: Option<bitcoin::transaction::Version>,
    pub fallback_lock_time: Option<bitcoin::locktime::absolute::LockTime>,
    pub xpubs: BTreeMap<bitcoin::bip32::Xpub, KeySource>,
    pub proprietaries: BTreeMap<raw::ProprietaryKey, Vec<u8>>,
    pub unknowns: BTreeMap<raw::Key, Vec<u8>>,
}

impl Join for Global {
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

// Perhaps we need more granular transaction states here:
// 1. UnorderedInputs
// 2. UnorderedOutputs
// 3. WithNoGlobal
// 4. Finished -> Convertable to a PSBTv2 with non modifiable fields enabled
// If we accumulate information pertaining to a previous state, we can transition back to the that state, sort and then progress again.
// merging semantics should be defined on each state. e.g merging a UnOrderedOutputs should not affect the inputs as they are already ordered.
pub enum Transaction {
    UnOrderedInputs(UnOrderedInputs),
    UnOrderedOutputs(OrderedInputs),
    WithNoGlobal(OrderedOutputs),
    ConvertableToPsbt(WithGlobal),
}

impl Transaction {
    pub fn new() -> UnOrderedInputs {
        UnOrderedInputs::default()
    }
}

#[derive(Default, Debug)]
pub struct UnOrderedInputs {
    inputs: HashSet<Vin>,
    outputs: HashSet<Vout>,
    global: Global,
}

impl Join for UnOrderedInputs {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            inputs: self.inputs.join(&other.inputs)?,
            outputs: self.outputs.join(&other.outputs)?,
            global: self.global.join(&other.global)?,
        })
    }
}

impl UnOrderedInputs {
    pub fn apply_bip69_ordering(self) -> OrderedInputs {
        let mut inputs = self.inputs.into_iter().collect::<Vec<_>>();
        inputs.sort_by_key(|input| (input.txid, input.vout));
        OrderedInputs {
            inputs,
            outputs: self.outputs.clone(),
            global: self.global,
        }
    }
    // TODO: sort using some salt as input to chacha20
}

#[derive(Default, Debug)]
pub struct OrderedInputs {
    inputs: Vec<Vin>,
    outputs: HashSet<Vout>,
    global: Global,
}

impl Join for OrderedInputs {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            inputs: self.inputs.clone(),
            outputs: self.outputs.join(&other.outputs)?,
            global: self.global.join(&other.global)?,
        })
    }
}

impl OrderedInputs {
    pub fn apply_bip69_ordering(self) -> OrderedOutputs {
        let mut outputs = self.outputs.into_iter().collect::<Vec<_>>();
        outputs.sort_by_key(|output| (output.value, output.script_pubkey.clone()));
        OrderedOutputs {
            inputs: self.inputs.clone(),
            outputs,
            global: self.global,
        }
    }
    // TODO: sort using some salt as input to chacha20
}

#[derive(Default, Debug)]
pub struct OrderedOutputs {
    inputs: Vec<Vin>,
    outputs: Vec<Vout>,
    global: Global,
}

impl Join for OrderedOutputs {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            inputs: self.inputs.clone(),
            outputs: self.outputs.clone(),
            global: self.global.join(&other.global)?,
        })
    }
}

impl OrderedOutputs {
    pub fn finalize(self) -> WithGlobal {
        WithGlobal {
            inputs: self.inputs,
            outputs: self.outputs,
            global: self.global,
        }
    }
}

#[derive(Default, Debug)]
pub struct WithGlobal {
    inputs: Vec<Vin>,
    outputs: Vec<Vout>,
    global: Global,
}

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

impl TryFrom<WithGlobal> for Psbt {
    type Error = PsbtConversionError;
    fn try_from(psbt: WithGlobal) -> Result<Self, Self::Error> {
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
                        previous_txid: input.txid.ok_or(PsbtConversionError::MissingOutpointTxid(i))?,
                        spent_output_index: input
                            .vout
                            .ok_or(PsbtConversionError::MissingOutpointVout(i))?,
                        sequence: input.sequence,
                        witness_utxo: input.prev_out,
                        final_script_sig: input.script_sig,
                        final_script_witness: input.witness,
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
                        amount: output
                            .value
                            .ok_or(PsbtConversionError::MissingValue(i))?,
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

#[derive(Default, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Vin {
    pub txid: Option<bitcoin::Txid>,
    pub vout: Option<u32>,
    pub script_sig: Option<bitcoin::ScriptBuf>,
    pub witness: Option<bitcoin::Witness>,
    pub sequence: Option<bitcoin::Sequence>,
    pub prev_out: Option<bitcoin::TxOut>,
    // TODO: extend to include all psbt inputs fields
}

impl Vin {
    pub fn from_input(input: &bitcoin::transaction::TxIn) -> Self {
        Self {
            txid: Some(input.previous_output.txid),
            vout: Some(input.previous_output.vout),
            script_sig: Some(input.script_sig.clone()),
            witness: Some(input.witness.clone()),
            sequence: Some(input.sequence),
            ..Default::default()
        }
    }

    pub fn with_prev_out(mut self, prev_out: bitcoin::TxOut) -> Self {
        self.prev_out = Some(prev_out);
        self
    }

    pub fn with_witness(mut self, witness: bitcoin::Witness) -> Self {
        self.witness = Some(witness);
        self
    }

    pub fn with_script_sig(mut self, script_sig: bitcoin::ScriptBuf) -> Self {
        self.script_sig = Some(script_sig);
        self
    }

    pub fn with_sequence(mut self, sequence: bitcoin::Sequence) -> Self {
        self.sequence = Some(sequence);
        self
    }

    pub fn with_vout(mut self, vout: u32) -> Self {
        self.vout = Some(vout);
        self
    }

    pub fn with_txid(mut self, txid: bitcoin::Txid) -> Self {
        self.txid = Some(txid);
        self
    }

    pub fn with_outpoint(mut self, outpoint: bitcoin::OutPoint) -> Self {
        self.txid = Some(outpoint.txid);
        self.vout = Some(outpoint.vout);
        self
    }
}

impl Join for Vin {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            txid: self.txid.join(&other.txid)?,
            vout: self.vout.join(&other.vout)?,
            script_sig: self.script_sig.join(&other.script_sig)?,
            witness: self.witness.join(&other.witness)?,
            sequence: self.sequence.join(&other.sequence)?,
            prev_out: self.prev_out.join(&other.prev_out)?,
        })
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_flow() {
        let mut tx = Transaction::new();
        let my_vin = Vin::from_input(&bitcoin::transaction::TxIn::default());
        tx.inputs.insert(my_vin.clone());

        let mut tx = tx.apply_bip69_ordering();
        let my_vout = Vout::from_output(&bitcoin::TxOut {
            value: bitcoin::Amount::from_sat(1000),
            script_pubkey: bitcoin::ScriptBuf::new(),
        });
        tx.outputs.insert(my_vout.clone());
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

        assert_eq!(psbt.inputs[0].previous_txid, my_vin.txid.unwrap());
        assert_eq!(psbt.inputs[0].spent_output_index, my_vin.vout.unwrap());
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
