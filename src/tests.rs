use crate::{
    global::Global,
    input::{Input, InputSet},
    output::{Output, OutputSet},
    partial_join::PartialJoin,
    tx::UnorderedPsbt,
};
use bitcoin::{Amount, OutPoint, ScriptBuf, TxOut, Txid, hashes::Hash};
use proptest::prelude::*;

prop_compose! {
    fn arb_input()(byte in any::<u8>(), vout in any::<u32>()) -> Input {
        let mut txid_bytes = [0u8; 32];
        txid_bytes[0] = byte;
        Input::new(&OutPoint::new(Txid::from_byte_array(txid_bytes), vout))
    }
}

prop_compose! {
    fn arb_output()(sats in any::<u64>(), script_byte in any::<u8>()) -> Output {
        Output::new(TxOut {
            value: Amount::from_sat(sats),
            script_pubkey: ScriptBuf::from(vec![script_byte]),
        })
    }
}

prop_compose! {
    fn arb_input_set()(inputs in proptest::collection::vec(arb_input(), 0..4)) -> InputSet {
        let mut s = InputSet::default();
        for i in &inputs { let _ = s.insert(i); }
        s
    }
}

prop_compose! {
    fn arb_output_set()(outputs in proptest::collection::vec(arb_output(), 0..4)) -> OutputSet {
        let mut s = OutputSet::default();
        for o in &outputs { let _ = s.insert(o); }
        s
    }
}

prop_compose! {
    fn arb_psbt()(inputs in arb_input_set(), outputs in arb_output_set()) -> UnorderedPsbt {
        let mut global = Global::default();
        global.input_count = inputs.len();
        global.output_count = outputs.len();
        UnorderedPsbt { global, inputs, outputs }
    }
}

macro_rules! laws {
    ($mod:ident, $ty:ty, $strategy:expr) => {
        mod $mod {
            use super::*;

            proptest! {
                #[test]
                fn idempotent(a in $strategy) {
                    prop_assert_eq!(a.join(&a), Ok(a.clone()));
                }

                #[test]
                fn commutative(a in $strategy, b in $strategy) {
                    let ab = a.join(&b);
                    let ba = b.join(&a);
                    prop_assert_eq!(ab, ba);
                }

                #[test]
                fn associative(a in $strategy, b in $strategy, c in $strategy) {
                    let left  = a.join(&b).and_then(|ab| ab.join(&c));
                    let right = b.join(&c).and_then(|bc| a.join(&bc));
                    prop_assert_eq!(left, right);
                }
            }
        }
    };
}

laws!(laws_u32, u32, any::<u32>());
laws!(
    laws_vec_u32,
    Vec<u32>,
    proptest::collection::vec(any::<u32>(), 0..8)
);
laws!(laws_input, Input, arb_input());
laws!(laws_output, Output, arb_output());
laws!(laws_option_u32, Option<u32>, any::<Option<u32>>());
laws!(
    laws_btreemap,
    std::collections::BTreeMap<u8, u32>,
    proptest::collection::btree_map(any::<u8>(), any::<u32>(), 0..8)
);
laws!(laws_input_set, InputSet, arb_input_set());
laws!(laws_output_set, OutputSet, arb_output_set());
laws!(laws_psbt, UnorderedPsbt, arb_psbt());

#[test]
fn psbt_global_count_recomputed() {
    let mut s = InputSet::default();
    let _ = s.insert(&Input::new(&OutPoint::new(
        Txid::from_byte_array([1u8; 32]),
        0,
    )));
    let mut p = UnorderedPsbt {
        global: Global::default(),
        inputs: s,
        outputs: OutputSet::default(),
    };
    p.global.input_count = 99;

    let joined = p.join(&p).unwrap();
    assert_eq!(joined.global.input_count, 1);
    assert_eq!(joined.global.output_count, 0);
}

// tap_key_origins maps each xpub to (Vec<TapLeafHash>, KeySource).
// BIP 371 says leaf hashes for the same xpub should be unioned across signers.
// Two signers that each control a different script path under the same internal key
// should join even though both views are valid.
#[test]
fn tap_key_origins_leaf_hashes_should_union() {
    let sk = SecretKey::from_slice(&[1u8; 32]).unwrap();
    let xpub = Keypair::from_secret_key(&Secp256k1::signing_only(), &sk)
        .x_only_public_key()
        .0;
    let outpoint = OutPoint::new(Txid::from_byte_array([0u8; 32]), 0);
    let key_source = (Fingerprint::from([0u8; 4]), DerivationPath::default());

    let leaf_a = TapLeafHash::from_byte_array([1u8; 32]);
    let leaf_b = TapLeafHash::from_byte_array([2u8; 32]);

    let mut input_a = Input::new(&outpoint);
    input_a
        .tap_key_origins
        .insert(xpub, (vec![leaf_a], key_source.clone()));

    let mut input_b = Input::new(&outpoint);
    input_b
        .tap_key_origins
        .insert(xpub, (vec![leaf_b], key_source.clone()));

    let result = input_a.join(&input_b);
    assert!(
        result.is_ok(),
        "joining inputs with same xpub but disjoint leaf hashes should succeed per BIP 371; got: {result:?}"
    );
    let (leaves, _) = result.unwrap().tap_key_origins.get(&xpub).unwrap().clone();
    assert!(leaves.contains(&leaf_a) && leaves.contains(&leaf_b));
}
