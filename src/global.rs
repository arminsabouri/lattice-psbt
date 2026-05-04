pub use psbt_v2::v2::Global;

use crate::partial_join::PartialJoin;
use crate::values::ValueError;

impl PartialJoin for Global {
    type Error = ValueError;

    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        // Self<Result<T>>
        // TODO input_count and output_count -> 0 if disagree? or must agree?
        Ok(Self {
            version: self.version.join(&other.version)?,
            tx_version: self.tx_version.join(&other.tx_version)?,
            fallback_lock_time: self.fallback_lock_time.join(&other.fallback_lock_time)?,
            xpubs: self.xpubs.join(&other.xpubs)?,
            proprietaries: self.proprietaries.join(&other.proprietaries)?,
            unknowns: self.unknowns.join(&other.unknowns)?,
            tx_modifiable_flags: self.tx_modifiable_flags.join(&other.tx_modifiable_flags)?,
            input_count: 0,
            output_count: 0,
        })
    }
}
