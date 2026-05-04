use crate::global::Global;
use crate::input::InputSet;
use crate::output::OutputSet;

use crate::partial_join::PartialJoin;
use crate::values::ValueError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnorderedPsbt {
    /// The global map.
    pub global: Global,
    /// The corresponding key-value map for each input in the unsigned transaction.
    pub inputs: InputSet,
    /// The corresponding key-value map for each output in the unsigned transaction.
    pub outputs: OutputSet,
}

impl PartialJoin for UnorderedPsbt {
    type Error = ValueError;

    fn join(&self, other: &Self) -> Result<Self, Self::Error> {
        let inputs = self.inputs.join(&other.inputs)?;
        let outputs = self.outputs.join(&other.outputs)?;

        let mut global = self.global.join(&other.global)?;
        global.input_count = inputs.len();
        global.output_count = outputs.len();

        Ok(Self {
            global,
            inputs,
            outputs,
        })
    }
}
