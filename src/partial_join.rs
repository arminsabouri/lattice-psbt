/// Trait for a fallible join operation.
pub trait PartialJoin {
    /// The error type for when `join` fails.
    type Error;

    /// Merge two values of a given type into a new value of the same type
    /// incorporating the information of both inputs.
    ///
    /// This operation should be associative, commutative and idempotent.
    fn join(&self, other: &Self) -> Result<Self, Self::Error>
    where
        Self: Sized;
}
