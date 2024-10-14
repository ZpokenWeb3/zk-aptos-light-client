//! An end-to-end example of using the SP1 SDK to generate a proof of a program that can be executed
//! or have a core proof generated.
//!
//! You can run this script using the following command:
//! ```shell
//! RUST_LOG=info cargo run --release -- --execute
//! ```
//! or
//! ```shell
//! RUST_LOG=info cargo run --release -- --prove
//! ```

use aptos_guests::EPOCH_CHANGE_ID;
use aptos_lc_core::aptos_test_utils::wrapper::AptosWrapper;
use aptos_lc_core::crypto::hash::CryptoHash;
use aptos_lc_core::types::trusted_state::TrustedState;
use host::epoch_change::prove_epoch_change;
use risc0_zkvm::default_prover;
use serde::Serialize;

const NBR_VALIDATORS: usize = 130;
const AVERAGE_SIGNERS_NBR: usize = 95;


struct ProvingAssets {
    trusted_state: Vec<u8>,
    validator_verifier_hash: Vec<u8>,
    epoch_change_proof: Vec<u8>,
}

impl ProvingAssets {
    /// Constructs a new instance of `ProvingAssets` by setting up the necessary state and proofs for the benchmark.
    fn new() -> Self {
        let mut aptos_wrapper = AptosWrapper::new(2, NBR_VALIDATORS, AVERAGE_SIGNERS_NBR).unwrap();

        let trusted_state = bcs::to_bytes(aptos_wrapper.trusted_state()).unwrap();
        let validator_verifier_hash = match TrustedState::from_bytes(&trusted_state).unwrap() {
            TrustedState::EpochState { epoch_state, .. } => epoch_state.verifier().hash().to_vec(),
            _ => panic!("Expected epoch change for current trusted state"),
        };
        let trusted_state_version = *aptos_wrapper.current_version();

        aptos_wrapper.generate_traffic().unwrap();

        let state_proof = aptos_wrapper
            .new_state_proof(trusted_state_version)
            .unwrap();

        let epoch_change_proof = &bcs::to_bytes(state_proof.epoch_changes()).unwrap();

        Self {
            trusted_state,
            validator_verifier_hash,
            epoch_change_proof: epoch_change_proof.clone(),
        }
    }
}

#[derive(Serialize)]
struct Timings {
    proving_time: u128,
    verifying_time: u128,
}


fn main() {
    // Setup the logger.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    let proving_assets = ProvingAssets::new();
    let prover = default_prover();
    let (receipt, output) = prove_epoch_change(&*prover, &proving_assets.trusted_state, &proving_assets.epoch_change_proof).unwrap();
    assert_eq!(
        output.prev_epoch_validator_verifier_hash,
        proving_assets.validator_verifier_hash.as_slice()
    );

    receipt
        .verify(EPOCH_CHANGE_ID)
        .unwrap();
}
