use psbt_v2::v2::Mod;
use psbt_v2::v2::Modifiable;

use crate::tx::UnorderedPsbt;
use std::marker::PhantomData;

/// Marker for a `Constructor` with both inputs and outputs unordered.
pub enum Unordered {}
/// Marker for a `Constructor` with inputs unordered.
pub enum InputsOnlyUnordered {}
/// Marker for a `Constructor` with outputs unordered.
pub enum OutputsOnlyUnordered {}

mod sealed {
    pub trait Unord {}
    impl Unord for super::Unordered {}
    impl Unord for super::InputsOnlyUnordered {}
    impl Unord for super::OutputsOnlyUnordered {}
}

/// Marker for if either inputs or outputs are unordered, or both.
pub trait Unord: sealed::Unord + Sync + Send + Sized + Unpin {}

impl Unord for Unordered {}
impl Unord for InputsOnlyUnordered {}
impl Unord for OutputsOnlyUnordered {}

/// Implements the  Constructor role.
///
///
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constructor<M, O>(UnorderedPsbt, PhantomData<(M, O)>);

impl<M: Mod, O: Unord> Constructor<M, O> {
    fn fix_input_order(&mut self) {
        todo!("set unordered inputs = false")
    }

    fn fix_output_order(&mut self) {
        todo!("set unordered outputs = false")
    }

    fn fix_order(&mut self) {
        todo!("set unordered = false")
    }

    // /// Extract PSBT that can be transferred to construct another constructor with it and resume
    // fn psbt()

    // /// Mark non-modifiable, fix ordering, and switch out of constructor role to updater role
    // fn updater(self) ->
}

impl Constructor<Modifiable, Unordered> {}
