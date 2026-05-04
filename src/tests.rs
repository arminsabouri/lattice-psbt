use crate::{
    global::Global,
    input::{Input, InputSet},
    output::{Output, OutputSet},
    partial_join::PartialJoin,
    tx::UnorderedPsbt,
};
use bitcoin::{
    Amount, OutPoint, PublicKey, ScriptBuf, Sequence, TapNodeHash, TxOut, Txid, Witness, absolute,
    bip32::{ChildNumber, DerivationPath, Fingerprint},
    hashes::{Hash, hash160, ripemd160, sha256, sha256d},
    secp256k1::{self, Keypair, Secp256k1, SecretKey, XOnlyPublicKey},
    sighash::{EcdsaSighashType, TapSighashType},
    taproot::TapLeafHash,
};
use proptest::prelude::*;

fn arb_script() -> impl Strategy<Value = ScriptBuf> {
    (0u8..4).prop_map(|b| ScriptBuf::from(vec![b]))
}

fn arb_txout() -> impl Strategy<Value = TxOut> {
    (0u64..4, arb_script()).prop_map(|(sats, script)| TxOut {
        value: Amount::from_sat(sats),
        script_pubkey: script,
    })
}

fn arb_height() -> impl Strategy<Value = absolute::Height> {
    (0u32..4).prop_map(|n| absolute::Height::from_consensus(n).expect("valid height"))
}

fn arb_time() -> impl Strategy<Value = absolute::Time> {
    (500_000_000u32..500_000_004)
        .prop_map(|n| absolute::Time::from_consensus(n).expect("valid time"))
}

fn arb_sighash_type() -> impl Strategy<Value = psbt_v2::PsbtSighashType> {
    prop_oneof![
        Just(psbt_v2::PsbtSighashType::from(EcdsaSighashType::All)),
        Just(psbt_v2::PsbtSighashType::from(EcdsaSighashType::None)),
        // TODO: single should be disallowed in the newer type
        Just(psbt_v2::PsbtSighashType::from(EcdsaSighashType::Single)),
    ]
}

fn arb_witness() -> impl Strategy<Value = Witness> {
    proptest::collection::vec(proptest::collection::vec(any::<u8>(), 0..4), 0..3)
        .prop_map(|vecs| Witness::from_slice(&vecs))
}

fn arb_ripemd160() -> impl Strategy<Value = ripemd160::Hash> {
    (0u8..4).prop_map(|n| ripemd160::Hash::from_byte_array([n; 20]))
}

fn arb_hash160() -> impl Strategy<Value = hash160::Hash> {
    (0u8..4).prop_map(|n| hash160::Hash::from_byte_array([n; 20]))
}

fn arb_sha256() -> impl Strategy<Value = sha256::Hash> {
    (0u8..4).prop_map(|n| sha256::Hash::from_byte_array([n; 32]))
}

fn arb_sha256d() -> impl Strategy<Value = sha256d::Hash> {
    (0u8..4).prop_map(|n| sha256d::Hash::from_byte_array([n; 32]))
}

fn arb_tap_leaf_hash() -> impl Strategy<Value = TapLeafHash> {
    (0u8..4).prop_map(|n| TapLeafHash::from_byte_array([n; 32]))
}

fn arb_tap_node_hash() -> impl Strategy<Value = TapNodeHash> {
    (0u8..4).prop_map(|n| TapNodeHash::from_byte_array([n; 32]))
}

fn arb_preimage_map<H>(
    arb_hash: impl Strategy<Value = H>,
) -> impl Strategy<Value = std::collections::BTreeMap<H, Vec<u8>>>
where
    H: Ord + Clone + std::fmt::Debug + 'static,
{
    proptest::collection::btree_map(arb_hash, proptest::collection::vec(0u8..4u8, 0..4), 0..3)
}

fn arb_secret_key() -> impl Strategy<Value = SecretKey> {
    (1u8..5).prop_map(|n| SecretKey::from_slice(&[n; 32]).expect("valid secret key"))
}

fn arb_secp256k1_pubkey() -> impl Strategy<Value = secp256k1::PublicKey> {
    arb_secret_key()
        .prop_map(|sk| secp256k1::PublicKey::from_secret_key(&Secp256k1::signing_only(), &sk))
}

fn arb_bitcoin_pubkey() -> impl Strategy<Value = PublicKey> {
    arb_secp256k1_pubkey().prop_map(PublicKey::new)
}

fn arb_xonly_pubkey() -> impl Strategy<Value = XOnlyPublicKey> {
    arb_secret_key().prop_map(|sk| {
        Keypair::from_secret_key(&Secp256k1::signing_only(), &sk)
            .x_only_public_key()
            .0
    })
}

fn arb_key_source() -> impl Strategy<Value = bitcoin::bip32::KeySource> {
    (any::<[u8; 4]>(), proptest::collection::vec(0u32..4, 0..3)).prop_map(
        |(fp_bytes, path_indices)| {
            let fingerprint = Fingerprint::from(fp_bytes);
            let path = DerivationPath::from_iter(
                path_indices
                    .into_iter()
                    .map(|n| ChildNumber::from_normal_idx(n).expect("valid child number")),
            );
            (fingerprint, path)
        },
    )
}

fn arb_ecdsa_sig() -> impl Strategy<Value = bitcoin::ecdsa::Signature> {
    arb_secret_key().prop_map(|sk| {
        let secp = Secp256k1::signing_only();
        let sig = secp.sign_ecdsa(&secp256k1::Message::from_digest([0u8; 32]), &sk);
        bitcoin::ecdsa::Signature {
            signature: sig,
            sighash_type: EcdsaSighashType::All,
        }
    })
}

fn arb_taproot_sig() -> impl Strategy<Value = bitcoin::taproot::Signature> {
    arb_secret_key().prop_map(|sk| {
        let secp = Secp256k1::signing_only();
        let kp = Keypair::from_secret_key(&secp, &sk);
        let sig = secp.sign_schnorr_no_aux_rand(&secp256k1::Message::from_digest([0u8; 32]), &kp);
        bitcoin::taproot::Signature {
            signature: sig,
            sighash_type: TapSighashType::Default,
        }
    })
}

fn arb_raw_key() -> impl Strategy<Value = psbt_v2::raw::Key> {
    (0u8..4, proptest::collection::vec(0u8..4u8, 0..4))
        .prop_map(|(type_value, key)| psbt_v2::raw::Key { type_value, key })
}

fn arb_proprietary_key() -> impl Strategy<Value = psbt_v2::raw::ProprietaryKey> {
    (
        proptest::collection::vec(0u8..4u8, 0..4),
        0u8..4,
        proptest::collection::vec(0u8..4u8, 0..4),
    )
        .prop_map(|(prefix, subtype, key)| psbt_v2::raw::ProprietaryKey {
            prefix,
            subtype,
            key,
        })
}

// Split across two prop_compose! calls to stay within tuple-size limits.
// arb_input_a covers scalar and script fields; arb_input adds map fields and crypto.
// Skipped: tap_script_sigs, tap_scripts, non_witness_utxo require ControlBlock
// or full Transaction construction.
prop_compose! {
    fn arb_input_a()(
        byte in 0u8..4,
        vout in 0u32..2,
        sequence         in proptest::option::of(0u32..4u32),
        min_height       in proptest::option::of(arb_height()),
        min_time         in proptest::option::of(arb_time()),
        witness_utxo     in proptest::option::of(arb_txout()),
        sighash_type     in proptest::option::of(arb_sighash_type()),
        redeem_script    in proptest::option::of(arb_script()),
        witness_script   in proptest::option::of(arb_script()),
        final_script_sig in proptest::option::of(arb_script()),
    ) -> Input {
        let mut txid_bytes = [0u8; 32];
        txid_bytes[0] = byte;
        let mut input = Input::new(&OutPoint::new(Txid::from_byte_array(txid_bytes), vout));
        input.sequence = sequence.map(Sequence);
        input.min_height = min_height;
        input.min_time = min_time;
        input.witness_utxo = witness_utxo;
        input.sighash_type = sighash_type;
        input.redeem_script = redeem_script;
        input.witness_script = witness_script;
        input.final_script_sig = final_script_sig;
        input
    }
}

prop_compose! {
    fn arb_input()(
        input                in arb_input_a(),
        final_script_witness in proptest::option::of(arb_witness()),
        tap_key_sig          in proptest::option::of(arb_taproot_sig()),
        tap_internal_key     in proptest::option::of(arb_xonly_pubkey()),
        tap_merkle_root      in proptest::option::of(arb_tap_node_hash()),
        partial_sigs      in proptest::collection::btree_map(arb_bitcoin_pubkey(), arb_ecdsa_sig(), 0..3),
        bip32_derivations in proptest::collection::btree_map(arb_secp256k1_pubkey(), arb_key_source(), 0..3),
        tap_key_origins   in proptest::collection::btree_map(
            arb_xonly_pubkey(),
            (proptest::collection::vec(arb_tap_leaf_hash(), 0..3), arb_key_source()),
            0..3,
        ),
        ripemd160_preimages in arb_preimage_map(arb_ripemd160()),
        sha256_preimages    in arb_preimage_map(arb_sha256()),
        hash160_preimages   in arb_preimage_map(arb_hash160()),
        hash256_preimages   in arb_preimage_map(arb_sha256d()),
        unknowns      in proptest::collection::btree_map(arb_raw_key(), proptest::collection::vec(0u8..4, 0..4), 0..3),
        proprietaries in proptest::collection::btree_map(arb_proprietary_key(), proptest::collection::vec(0u8..4, 0..4), 0..3),
    ) -> Input {
        let mut input = input;
        input.final_script_witness = final_script_witness;
        input.tap_key_sig = tap_key_sig;
        input.tap_internal_key = tap_internal_key;
        input.tap_merkle_root = tap_merkle_root;
        input.partial_sigs = partial_sigs;
        input.bip32_derivations = bip32_derivations;
        input.tap_key_origins = tap_key_origins;
        input.ripemd160_preimages = ripemd160_preimages;
        input.sha256_preimages = sha256_preimages;
        input.hash160_preimages = hash160_preimages;
        input.hash256_preimages = hash256_preimages;
        input.unknowns = unknowns;
        input.proprietaries = proprietaries;
        input
    }
}

prop_compose! {
    // TODO: Skipped: tap_tree: requires building a full TapTree structure.
    fn arb_output()(
        sats              in 0u64..4,
        script_byte       in 0u8..4,
        redeem_script     in proptest::option::of(arb_script()),
        witness_script    in proptest::option::of(arb_script()),
        tap_internal_key  in proptest::option::of(arb_xonly_pubkey()),
        bip32_derivations in proptest::collection::btree_map(arb_secp256k1_pubkey(), arb_key_source(), 0..3),
        tap_key_origins   in proptest::collection::btree_map(
            arb_xonly_pubkey(),
            (proptest::collection::vec(arb_tap_leaf_hash(), 0..3), arb_key_source()),
            0..3,
        ),
        unknowns      in proptest::collection::btree_map(arb_raw_key(), proptest::collection::vec(0u8..4, 0..4), 0..3),
        proprietaries in proptest::collection::btree_map(arb_proprietary_key(), proptest::collection::vec(0u8..4, 0..4), 0..3),
    ) -> Output {
        let mut output = Output::new(TxOut {
            value: Amount::from_sat(sats),
            script_pubkey: ScriptBuf::from(vec![script_byte]),
        });
        output.redeem_script = redeem_script;
        output.witness_script = witness_script;
        output.tap_internal_key = tap_internal_key;
        output.bip32_derivations = bip32_derivations;
        output.tap_key_origins = tap_key_origins;
        output.unknowns = unknowns;
        output.proprietaries = proprietaries;
        output
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

// TODO: test hashmap joins
// laws!(
//     laws_hashmap_32,
//     std::collections::HashMap<bitcoin::ScriptBuf, Output>,
//     proptest::collection::hash_map(any::<u32>(), arb_output(), 0..8)
// );

prop_compose! {
    fn arb_global()(
        input_count  in 0usize..4,
        output_count in 0usize..4,
        fallback_lock_time in proptest::option::of(prop_oneof![
            arb_height().prop_map(|h| absolute::LockTime::Blocks(h)),
            arb_time().prop_map(|t| absolute::LockTime::Seconds(t)),
        ]),
        tx_modifiable_flags in 0u8..4,
        unknowns      in proptest::collection::btree_map(arb_raw_key(), proptest::collection::vec(0u8..4, 0..4), 0..3),
        proprietaries in proptest::collection::btree_map(arb_proprietary_key(), proptest::collection::vec(0u8..4, 0..4), 0..3),
    ) -> Global {
        let mut g = Global::default();
        g.input_count = input_count;
        g.output_count = output_count;
        g.fallback_lock_time = fallback_lock_time;
        g.tx_modifiable_flags = tx_modifiable_flags;
        g.unknowns = unknowns;
        g.proprietaries = proprietaries;
        g
    }
}

laws!(laws_global, Global, arb_global());
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
