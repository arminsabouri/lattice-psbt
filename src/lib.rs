use psbt_v2::v2::Psbt;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/*
Our goals: Create a monotone datastructure that can take un ordered transaction components can merge or joins them if they are non-conflicting.
Such a datastructure should have eventual consistency. We will model a PSBT as a meet-semilattice set.
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

macro_rules! impl_join_for_hashset {
    ($type:ty) => {
        impl Join for HashSet<$type> {
            fn join(&self, other: &Self) -> Result<Self, JoinError> {
                match (self, other) {
                    (a, b) if a.is_empty() || b.is_empty() => Ok(a.clone()),
                    (a, b) => {
                        let mut result = a.clone();
                        for item in b {
                            result.insert(item.clone());
                        }
                        Ok(result)
                    }
                }
            }
        }
    };
}

impl_join_for_hashset!(Vin);
impl_join_for_hashset!(Vout);

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

pub enum Transaction {
    UnOrderedTransaction(UnOrderedTransaction),
    OrderedTransaction(OrderedTransaction),
}

#[derive(Default, Debug)]
pub struct UnOrderedTransaction {
    inputs: HashSet<Vin>,
    outputs: HashSet<Vout>,
    nlocktime: Option<bitcoin::locktime::absolute::LockTime>,
    nversion: Option<bitcoin::transaction::Version>,
}

impl UnOrderedTransaction {
    pub fn from_transaction(transaction: bitcoin::Transaction) -> Self {
        Self {
            inputs: transaction
                .input
                .iter()
                .map(|input| Vin::from_input(input))
                .collect(),
            outputs: transaction
                .output
                .iter()
                .map(|output| Vout::from_output(output))
                .collect(),
            nlocktime: Some(transaction.lock_time),
            nversion: Some(transaction.version),
        }
    }

    pub fn add_input(&mut self, input: Vin) {
        self.inputs.insert(input);
    }

    pub fn add_output(&mut self, output: Vout) {
        self.outputs.insert(output);
    }

    pub fn with_nlocktime(mut self, nlocktime: bitcoin::locktime::absolute::LockTime) -> Self {
        self.nlocktime = Some(nlocktime);
        self
    }

    pub fn with_nversion(mut self, nversion: bitcoin::transaction::Version) -> Self {
        self.nversion = Some(nversion);
        self
    }
}

impl Join for UnOrderedTransaction {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            inputs: self.inputs.join(&other.inputs)?,
            outputs: self.outputs.join(&other.outputs)?,
            nlocktime: self.nlocktime.join(&other.nlocktime)?,
            nversion: self.nversion.join(&other.nversion)?,
        })
    }
}

#[derive(Default, Debug)]
pub struct OrderedTransaction {
    // TODO: this should be vec and ordering should be defined on an index
    // State machine should reflect ordered inputs, then ordered outputs.
    inputs: BTreeSet<Vin>,
    outputs: BTreeSet<Vout>,
    nlocktime: Option<bitcoin::locktime::absolute::LockTime>,
    nversion: Option<bitcoin::transaction::Version>,
}

impl From<UnOrderedTransaction> for OrderedTransaction {
    fn from(unordered: UnOrderedTransaction) -> Self {
        Self {
            inputs: unordered.inputs.into_iter().collect(),
            outputs: unordered.outputs.into_iter().collect(),
            nlocktime: unordered.nlocktime,
            nversion: unordered.nversion,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PsbtConversionError {
    #[error("Transaction is not valid")]
    InvalidTransaction,
}

impl TryFrom<OrderedTransaction> for Psbt {
    type Error = PsbtConversionError;
    fn try_from(ordered: OrderedTransaction) -> Result<Self, Self::Error> {
        let tx = psbt_v2::v2::Psbt {
            global: psbt_v2::v2::Global {
                tx_version: ordered
                    .nversion
                    .ok_or(PsbtConversionError::InvalidTransaction)?,
                fallback_lock_time: ordered.nlocktime,
                ..Default::default()
            },
            inputs: ordered
                .inputs
                .into_iter()
                .map(|input| {
                    Ok::<psbt_v2::v2::Input, PsbtConversionError>(psbt_v2::v2::Input {
                        previous_txid: input.txid.ok_or(PsbtConversionError::InvalidTransaction)?,
                        spent_output_index: input
                            .vout
                            .ok_or(PsbtConversionError::InvalidTransaction)?,
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
            outputs: ordered
                .outputs
                .into_iter()
                .map(|output| {
                    Ok::<psbt_v2::v2::Output, PsbtConversionError>(psbt_v2::v2::Output {
                        amount: output
                            .value
                            .ok_or(PsbtConversionError::InvalidTransaction)?,
                        script_pubkey: output
                            .script_pubkey
                            .ok_or(PsbtConversionError::InvalidTransaction)?,
                        ..Default::default()
                    })
                })
                .collect::<Result<Vec<_>, PsbtConversionError>>()?,
        };

        Ok(tx)
    }
}

#[derive(Default, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Debug)]
pub struct Vin {
    pub txid: Option<bitcoin::Txid>,
    pub vout: Option<u32>,
    pub script_sig: Option<bitcoin::ScriptBuf>,
    pub witness: Option<bitcoin::Witness>,
    pub sequence: Option<bitcoin::Sequence>,
    pub prev_out: Option<bitcoin::TxOut>,
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

#[derive(Clone, Default, PartialEq, Eq, Hash, Ord, PartialOrd, Debug)]
pub struct Vout {
    pub value: Option<bitcoin::Amount>,
    pub script_pubkey: Option<bitcoin::ScriptBuf>,
}

impl Vout {
    pub fn from_output(output: &bitcoin::transaction::TxOut) -> Self {
        Self {
            value: Some(output.value),
            script_pubkey: Some(output.script_pubkey.clone()),
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
        })
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // impl Arbitrary for UnOrderedTransaction {
    //     type Parameters = ();
    //     type Strategy = proptest::strategy::BoxedStrategy<Self>;

    //     fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
    //         (
    //             proptest::collection::hash_set(any::<Vin>(), 0..10),
    //             proptest::collection::hash_set(any::<Vout>(), 0..10),
    //             any::<Option<bitcoin::locktime::absolute::LockTime>>(),
    //             any::<Option<bitcoin::transaction::Version>>(),
    //         )
    //             .prop_map(|(inputs, outputs, nlocktime, nversion)| Self {
    //                 inputs,
    //                 outputs,
    //                 nlocktime,
    //                 nversion,
    //             })
    //             .boxed()
    //     }
    // }

    // impl Arbitrary for Vin {
    //     type Parameters = ();
    //     type Strategy = proptest::strategy::BoxedStrategy<Self>;

    //     fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
    //         (
    //             any::<Option<bitcoin::Txid>>(),
    //             any::<Option<u32>>(),
    //             any::<Option<bitcoin::ScriptBuf>>(),
    //             any::<Option<bitcoin::Witness>>(),
    //             any::<Option<bitcoin::Sequence>>(),
    //             any::<Option<bitcoin::TxOut>>(),
    //         )
    //             .prop_map(|(txid, vout, script_sig, witness, sequence, prev_out)| Self {
    //                 txid,
    //                 vout,
    //                 script_sig,
    //                 witness,
    //                 sequence,
    //                 prev_out,
    //             })
    //             .boxed()
    //     }
    // }

    impl Arbitrary for Vout {
        type Parameters = ();
        type Strategy = proptest::strategy::BoxedStrategy<Self>;

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            (
                any::<Option<u64>>().prop_map(|v| v.map(bitcoin::Amount::from_sat)),
                any::<Option<Vec<u8>>>().prop_map(|v| v.map(bitcoin::ScriptBuf::from)),
            )
                .prop_map(|(value, script_pubkey)| Self {
                    value,
                    script_pubkey,
                })
                .boxed()
        }
    }

    proptest! {
        #[test]
        fn test_join_vout(a: Vout, b: Vout) {
            let result = a.join(&b);
            
            // Test the join properties
            match (&a.value, &b.value) {
                (Some(x), Some(y)) if x != y => {
                    assert!(result.is_err());
                    assert_eq!(result.as_ref().unwrap_err(), &JoinError::ScalarDisagree);
                }
                _ => {
                    println!("a: {:?}, b: {:?}", a, b);
                    // Should succeed when values are same or one is None
                    let joined = result.as_ref().unwrap();
                    assert_eq!(joined.value, a.value.join(&b.value).unwrap());
                }
            }
            
            match (&a.script_pubkey, &b.script_pubkey) {
                (Some(x), Some(y)) if x != y => {
                    assert!(result.is_err());
                    assert_eq!(result.unwrap_err(), JoinError::ScalarDisagree);
                }
                _ => {
                    // Should succeed when scripts are same or one is None
                    let joined = result.unwrap();
                    assert_eq!(joined.script_pubkey, a.script_pubkey.join(&b.script_pubkey).unwrap());
                }
            }
        }
    }
}
