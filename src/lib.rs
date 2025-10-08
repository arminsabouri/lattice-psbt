use std::collections::{BTreeSet, HashSet};

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

macro_rules! impl_semi_lattice_field_value {
    ($t:ty) => {
        impl Join for Option<$t> {
            fn join(&self, other: &Self) -> Result<Self, JoinError> {
                match (self, other) {
                    (None, x) | (x, None) => Ok(x.clone()),
                    (Some(a), Some(b)) if a == b => Ok(Some(a.clone())),
                    _ => Err(JoinError::ScalarDisagree),
                }
            }
        }
    };
}

impl_semi_lattice_field_value!(u32);
impl_semi_lattice_field_value!(bitcoin::Txid);
impl_semi_lattice_field_value!(bitcoin::ScriptBuf);
impl_semi_lattice_field_value!(bitcoin::Witness);
impl_semi_lattice_field_value!(bitcoin::TxOut);
impl_semi_lattice_field_value!(bitcoin::Amount);
impl_semi_lattice_field_value!(bitcoin::Sequence);
impl_semi_lattice_field_value!(bitcoin::locktime::absolute::LockTime);
impl_semi_lattice_field_value!(bitcoin::transaction::Version);

macro_rules! impl_semi_lattice_for_hashset {
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
                    _ => Err(JoinError::StructuralMismatch),
                }
            }
        }
    };
}

impl_semi_lattice_for_hashset!(Vin);
impl_semi_lattice_for_hashset!(Vout);

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum JoinError {
    #[error("Scalar disagree")]
    ScalarDisagree,
    #[error("Structural mismatch / key collision")]
    StructuralMismatch,
}

trait Join {
    fn join(&self, other: &Self) -> Result<Self, JoinError>
    where
        Self: Sized;
}

pub enum Transaction {
    UnOrderedTransaction(UnOrderedTransaction),
    OrderedTransaction(OrderedTransaction),
}

#[derive(Default)]
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

pub struct OrderedTransaction {
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

#[derive(Default, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
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

#[derive(Clone, Default, PartialEq, Eq, Hash, Ord, PartialOrd)]
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
}

impl Join for Vout {
    fn join(&self, other: &Self) -> Result<Self, JoinError> {
        Ok(Self {
            value: self.value.join(&other.value)?,
            script_pubkey: self.script_pubkey.join(&other.script_pubkey)?,
        })
    }
}
