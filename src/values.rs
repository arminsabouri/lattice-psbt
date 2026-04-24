use crate::partial_join::PartialJoin;

// just replace with struct Conflict?
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValueError {
    #[error("conflict")]
    Conflict, // TODO Conflict(&T, &T) ?
}

impl<T> PartialJoin for T
where
    T: Idempotent + Clone,
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

/// A marker trait for value types that should have a trivial join based on equality
trait Idempotent: PartialEq {}

impl Idempotent for u32 {}
impl Idempotent for u8 {}
impl Idempotent for usize {}
// impl Idempotent for Vec<u8> {}
// impl Idempotent for Vec<bitcoin::TapLeafHash> {}
impl Idempotent for bitcoin::Txid {}
impl Idempotent for bitcoin::ScriptBuf {}
impl Idempotent for bitcoin::Witness {}
impl Idempotent for bitcoin::TxOut {}
impl Idempotent for bitcoin::Amount {}
impl Idempotent for bitcoin::Sequence {}
impl Idempotent for bitcoin::locktime::absolute::LockTime {}
impl Idempotent for bitcoin::transaction::Version {}
impl Idempotent for bitcoin::secp256k1::XOnlyPublicKey {}
impl Idempotent for bitcoin::taproot::TapTree {}
impl Idempotent for bitcoin::taproot::LeafVersion {}
impl Idempotent for bitcoin::TapLeafHash {}
impl Idempotent for psbt_v2::Version {}
impl Idempotent for bitcoin::absolute::Time {}
impl Idempotent for bitcoin::absolute::Height {}
impl Idempotent for bitcoin::Transaction {}
impl Idempotent for psbt_v2::PsbtSighashType {}
impl Idempotent for bitcoin::taproot::Signature {}
impl Idempotent for bitcoin::ecdsa::Signature {}
impl Idempotent for bitcoin::TapNodeHash {}
impl Idempotent for bitcoin::bip32::KeySource {}
