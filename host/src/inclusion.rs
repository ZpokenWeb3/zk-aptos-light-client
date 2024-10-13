use crate::error::LightClientError;
use aptos_lc_core::types::output::InclusionOutput;
use getset::Getters;
use risc0_zkvm::{ExecutorEnv, Prover, Receipt};
use serde::{Deserialize, Serialize};
use aptos_guests::INCLUSION_ELF;


#[derive(Clone, Debug, Getters, Serialize, Deserialize)]
#[getset(get = "pub")]
pub struct SparseMerkleProofAssets {
    sparse_merkle_proof: Vec<u8>,
    leaf_key: [u8; 32],
    leaf_hash: [u8; 32],
}

impl SparseMerkleProofAssets {
    pub const fn new(
        sparse_merkle_proof: Vec<u8>,
        leaf_key: [u8; 32],
        leaf_hash: [u8; 32],
    ) -> SparseMerkleProofAssets {
        SparseMerkleProofAssets {
            sparse_merkle_proof,
            leaf_key,
            leaf_hash,
        }
    }
}

#[derive(Clone, Debug, Getters, Serialize, Deserialize)]
#[getset(get = "pub")]
pub struct TransactionProofAssets {
    transaction: Vec<u8>,
    transaction_index: u64,
    transaction_proof: Vec<u8>,
    latest_li: Vec<u8>,
}

impl TransactionProofAssets {
    pub const fn new(
        transaction: Vec<u8>,
        transaction_index: u64,
        transaction_proof: Vec<u8>,
        latest_li: Vec<u8>,
    ) -> TransactionProofAssets {
        TransactionProofAssets {
            transaction,
            transaction_index,
            transaction_proof,
            latest_li,
        }
    }
}

#[derive(Clone, Debug, Getters, Serialize, Deserialize)]
#[getset(get = "pub")]
pub struct ValidatorVerifierAssets {
    validator_verifier: Vec<u8>,
}

impl ValidatorVerifierAssets {
    pub const fn new(validator_verifier: Vec<u8>) -> ValidatorVerifierAssets {
        ValidatorVerifierAssets { validator_verifier }
    }
}

pub fn generate_stdin<'a>(
    sparse_merkle_proof_assets: &'a SparseMerkleProofAssets,
    transaction_proof_assets: &'a TransactionProofAssets,
    validator_verifier_assets: &'a ValidatorVerifierAssets,
) -> ExecutorEnv<'a> {
    ExecutorEnv::builder()
        .write(&sparse_merkle_proof_assets.sparse_merkle_proof)
        .unwrap()
        .write(&sparse_merkle_proof_assets.leaf_key)
        .unwrap()
        .write(&sparse_merkle_proof_assets.leaf_hash)
        .unwrap()
        .write(&transaction_proof_assets.transaction)
        .unwrap()
        .write(&transaction_proof_assets.transaction_index)
        .unwrap()
        .write(&transaction_proof_assets.transaction_proof)
        .unwrap()
        .write(&transaction_proof_assets.latest_li)
        .unwrap()
        .write(&validator_verifier_assets.validator_verifier)
        .unwrap()
        .build()
        .unwrap()
}

#[allow(dead_code)]
pub fn prove_inclusion(
    client: &dyn Prover,
    sparse_merkle_proof_assets: &SparseMerkleProofAssets,
    transaction_proof_assets: &TransactionProofAssets,
    validator_verifier_assets: &ValidatorVerifierAssets,
) -> Result<(Receipt, InclusionOutput), LightClientError> {

    let env = generate_stdin(
        sparse_merkle_proof_assets,
        transaction_proof_assets,
        validator_verifier_assets,
    );

    let proof =
        client
            .prove(env, INCLUSION_ELF)
            .map_err(|err| LightClientError::ProvingError {
                program: "prove-merkle-inclusion".to_string(),
                source: err.into(),
            })?;

    // Read output.
    let output: InclusionOutput = proof.receipt.journal.decode().map_err(|err| LightClientError::DecodeError {
        program: "prove-merkle-inclusion".to_string(),
        source: err.into(),
    })?;

    Ok((
        proof.receipt,
        output,
    ))
}

