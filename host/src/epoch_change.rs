use crate::error::LightClientError;
use aptos_lc_core::types::output::EpochChangeOutput;
use risc0_zkvm::{ExecutorEnv, Prover, Receipt};
use aptos_guests::EPOCH_CHANGE_ELF;

pub fn generate_stdin<'a>(current_trusted_state: &'a [u8], epoch_change_proof: &'a [u8]) -> ExecutorEnv<'a> {
    ExecutorEnv::builder()
        .write(&current_trusted_state.to_vec())
        .unwrap()
        .write(&epoch_change_proof.to_vec())
        .unwrap()
        .build()
        .unwrap()
}

#[allow(dead_code)]
pub fn prove_epoch_change(
    client: &dyn Prover,
    trusted_state: &[u8],
    epoch_change_proof: &[u8],
) -> Result<(Receipt, EpochChangeOutput), LightClientError> {

    let env = generate_stdin(
        trusted_state,
        epoch_change_proof,
    );

    let mut proof =
        client
            .prove(env, EPOCH_CHANGE_ELF)
            .map_err(|err| LightClientError::ProvingError {
                program: "prove-epoch-change".to_string(),
                source: err.into(),
            })?;

    // Read output.
    let output: EpochChangeOutput = proof.receipt.journal.decode().map_err(|err| LightClientError::DecodeError {
        program: "prove-epoch-change".to_string(),
        source: err.into(),
    })?;

    Ok((
        proof.receipt,
        output,
    ))
}