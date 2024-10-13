use risc0_zkvm::default_prover;
use serde::Serialize;
use std::time::Instant;

use aptos_guests::INCLUSION_ID;
use aptos_lc_core::aptos_test_utils::wrapper::AptosWrapper;
use aptos_lc_core::crypto::hash::CryptoHash;
use aptos_lc_core::types::ledger_info::LedgerInfoWithSignatures;
use aptos_lc_core::types::trusted_state::TrustedState;
use aptos_lc_core::types::validator::ValidatorVerifier;
use host::inclusion::{prove_inclusion, SparseMerkleProofAssets, TransactionProofAssets, ValidatorVerifierAssets};
const NBR_LEAVES: [usize; 5] = [32, 128, 2048, 8192, 32768];
const NBR_VALIDATORS: usize = 130;
const AVERAGE_SIGNERS_NBR: usize = 95;

struct ProvingAssets {
    sparse_merkle_proof_assets: SparseMerkleProofAssets,
    transaction_proof_assets: TransactionProofAssets,
    validator_verifier_assets: ValidatorVerifierAssets,
    state_checkpoint_hash: [u8; 32],
}

impl ProvingAssets {
    fn from_nbr_leaves(nbr_leaves: usize) -> Self {
        let mut aptos_wrapper =
            AptosWrapper::new(nbr_leaves, NBR_VALIDATORS, AVERAGE_SIGNERS_NBR).unwrap();
        aptos_wrapper.generate_traffic().unwrap();

        let trusted_state = bcs::to_bytes(aptos_wrapper.trusted_state()).unwrap();
        let validator_verifier = match TrustedState::from_bytes(&trusted_state).unwrap() {
            TrustedState::EpochState { epoch_state, .. } => epoch_state.verifier().clone(),
            _ => panic!("expected epoch state"),
        };

        let proof_assets = aptos_wrapper
            .get_latest_proof_account(nbr_leaves - 1)
            .unwrap();

        let sparse_merkle_proof = bcs::to_bytes(proof_assets.state_proof()).unwrap();
        let key: [u8; 32] = *proof_assets.key().as_ref();
        let element_hash: [u8; 32] = *proof_assets.state_value_hash().unwrap().as_ref();

        let transaction = bcs::to_bytes(&proof_assets.transaction()).unwrap();
        let transaction_proof = bcs::to_bytes(&proof_assets.transaction_proof()).unwrap();
        let latest_li = aptos_wrapper.get_latest_li_bytes().unwrap();

        let sparse_merkle_proof_assets =
            SparseMerkleProofAssets::new(sparse_merkle_proof, key, element_hash);

        let state_checkpoint_hash = proof_assets
            .transaction()
            .ensure_state_checkpoint_hash()
            .unwrap();

        let transaction_proof_assets = TransactionProofAssets::new(
            transaction,
            *proof_assets.transaction_version(),
            transaction_proof,
            latest_li,
        );

        let validator_verifier_assets = ValidatorVerifierAssets::new(validator_verifier.to_bytes());

        Self {
            sparse_merkle_proof_assets,
            transaction_proof_assets,
            validator_verifier_assets,
            state_checkpoint_hash: *state_checkpoint_hash.as_ref(),
        }
    }
}

#[derive(Serialize)]
struct Timings {
    nbr_leaves: usize,
    proving_time: u128,
    verifying_time: u128,
}

fn main() {
    // Setup the logger.
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .init();

    for nbr_leaves in NBR_LEAVES {
        let proving_assets = ProvingAssets::from_nbr_leaves(nbr_leaves);
        let start_proving = Instant::now();
        let prover = default_prover();
        let (receipt, output) = prove_inclusion(&*prover, &proving_assets.sparse_merkle_proof_assets, &proving_assets.transaction_proof_assets,
                                                &proving_assets.validator_verifier_assets).unwrap();
        let proving_time = start_proving.elapsed();

        let start_verifying = Instant::now();
        receipt.verify(INCLUSION_ID).unwrap();
        let verifying_time = start_verifying.elapsed();

        // Verify the consistency of the validator verifier hash post-merkle proof.
        // This verifies the validator consistency required by P1.
        assert_eq!(
            &output.validator_verifier_hash,
            ValidatorVerifier::from_bytes(
                proving_assets
                    .validator_verifier_assets
                    .validator_verifier()
            )
                .unwrap()
                .hash()
                .as_ref()
        );

        // Verify the consistency of the final merkle root hash computed
        // by the program against the expected one.
        // This verifies P3 out-of-circuit.
        assert_eq!(
            output.reconstructed_root_hash, proving_assets.state_checkpoint_hash,
            "Merkle root hash mismatch"
        );

        let lates_li = proving_assets.transaction_proof_assets.latest_li();
        let expected_block_id = LedgerInfoWithSignatures::from_bytes(lates_li)
            .unwrap()
            .ledger_info()
            .block_id();
        assert_eq!(
            output.current_block_id.to_vec(),
            expected_block_id.to_vec(),
            "Block hash mismatch"
        );

        assert_eq!(
            output.key.to_vec(),
            proving_assets.sparse_merkle_proof_assets.leaf_key(),
            "Merkle tree key mismatch"
        );

        assert_eq!(
            output.leaf_value_hash.to_vec(),
            proving_assets.sparse_merkle_proof_assets.leaf_hash(),
            "Merkle tree value mismatch"
        );

        let timings = Timings {
            nbr_leaves,
            proving_time: proving_time.as_millis(),
            verifying_time: verifying_time.as_millis(),
        };

        let json_output = serde_json::to_string(&timings).unwrap();
        println!("{}", json_output);
    }
}
