use crate::{Join, JoinError};

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
