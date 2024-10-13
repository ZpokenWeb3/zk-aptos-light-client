use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InclusionOutput {
    pub validator_verifier_hash: [u8; 32],
    pub reconstructed_root_hash: [u8; 32],
    pub current_block_id: [u8; 32],
    pub key: [u8; 32],
    pub leaf_value_hash: [u8; 32],
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpochChangeOutput {
    pub prev_epoch_validator_verifier_hash: [u8; 32],
    pub validator_verifier_hash: [u8; 32],
}