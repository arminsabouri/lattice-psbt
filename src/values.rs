use crate::partial_join::PartialJoin;

// TODO just replace with struct Conflict()?
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValueError {
    #[error("Values differ")]
    Conflict, // TODO is Conflict(&T, &T) possible/reasonable? needs to borrow, should be ok if the both PSBTs are borrowed w/ same lifetime as error. or Box<dyn> by cloning?
}

/// A marker trait for value types that should have a trivial join based on equality
trait IdempotentValue: PartialEq {}

impl<T> PartialJoin for T
where
    T: IdempotentValue + Clone,
{
    type Error = ValueError;

    /// A join that succeeds when the two values are identical and fails otherwise
    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        if self == other {
            Ok(self.clone())
        } else {
            Err(ValueError::Conflict)
        }
    }
}

// struct IdempotentValueLattice<T : Clone + PartialEq>(T) + impl<T> PartialJoin for IdempotentValue<T>?
// or just blanket impl for PartialEq + Clone?
//
// QuotientLattice -> pick arbitrarily? not partial

impl IdempotentValue for u32 {}
impl IdempotentValue for u8 {}
impl IdempotentValue for usize {}

impl IdempotentValue for bitcoin::absolute::Height {}
impl IdempotentValue for bitcoin::absolute::Time {}
impl IdempotentValue for bitcoin::Amount {}
impl IdempotentValue for bitcoin::bip32::KeySource {}
impl IdempotentValue for bitcoin::ecdsa::Signature {}
impl IdempotentValue for bitcoin::locktime::absolute::LockTime {}
impl IdempotentValue for bitcoin::ScriptBuf {}
impl IdempotentValue for bitcoin::secp256k1::XOnlyPublicKey {}
impl IdempotentValue for bitcoin::Sequence {}
impl IdempotentValue for bitcoin::TapLeafHash {}
impl IdempotentValue for bitcoin::TapNodeHash {}
impl IdempotentValue for bitcoin::taproot::LeafVersion {}
impl IdempotentValue for bitcoin::taproot::Signature {}
impl IdempotentValue for bitcoin::taproot::TapTree {}
impl IdempotentValue for bitcoin::Transaction {}
impl IdempotentValue for bitcoin::transaction::Version {}
impl IdempotentValue for bitcoin::Txid {}
impl IdempotentValue for bitcoin::TxOut {}
impl IdempotentValue for bitcoin::Witness {}

impl IdempotentValue for psbt_v2::PsbtSighashType {}
impl IdempotentValue for psbt_v2::Version {}
