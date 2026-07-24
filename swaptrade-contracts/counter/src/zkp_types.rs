/// Zero-Knowledge Proof Types for Private Transactions
///
/// This module defines the core data structures and types used throughout
/// the ZKP system for private transaction validation and verification.
use soroban_sdk::{contracttype, Bytes};

/// Represents a cryptographic commitment to a hidden value
/// Used for committing to amounts, balances, or other sensitive data
#[contracttype]
#[derive(Clone, Debug)]
pub struct Commitment {
    /// The commitment value (e.g., Pedersen commitment)
    pub value: Bytes,
}

/// Represents a zero-knowledge proof of knowledge
/// Generic proof structure that can represent different ZKP schemes
#[contracttype]
#[derive(Clone, Debug)]
pub struct ZKProof {
    /// The proof data
    pub proof_data: Bytes,
    /// Type of ZKP scheme used
    pub scheme: ProofScheme,
}

/// Supported Zero-Knowledge Proof Schemes
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofScheme {
    /// Bulletproofs for efficient range proofs
    Bulletproof,
    /// zk-SNARK for general circuit satisfaction
    ZkSnark,
    /// Simplified proof scheme for testing/fallback
    SimplifiedProof,
}

/// Range Proof for proving a value is within a specific range
/// Used to prove transaction amounts without revealing them
#[contracttype]
#[derive(Clone, Debug)]
pub struct RangeProof {
    /// The actual range proof
    pub proof: Bytes,
    /// Commitment to the value being proven
    pub commitment: Bytes,
    /// Bit length of the range (e.g., 64 for amounts)
    pub bit_length: u32,
}

/// Balance Proof proving a user's balance meets certain conditions
/// without revealing the exact balance
#[contracttype]
#[derive(Clone, Debug)]
pub struct BalanceProof {
    /// Commitment to the balance
    pub balance_commitment: Bytes,
    /// Proof of sufficient balance for transaction
    pub sufficiency_proof: Bytes,
    /// Timestamp when proof was generated
    pub proof_timestamp: u64,
}

/// Transaction Witness containing the private values needed to generate proofs
/// Generated off-chain, kept private, used to generate proofs
#[contracttype]
#[derive(Clone, Debug)]
pub struct TransactionWitness {
    /// The amount being transferred (private)
    pub amount: i128,
    /// Blinding factor for the amount commitment
    pub amount_blinding: Bytes,
    /// Random nonce for proof
    pub nonce: Bytes,
    /// Sender's balance (private)
    pub sender_balance: i128,
    /// Balance blinding factor
    pub balance_blinding: Bytes,
}

/// Private Transaction hiding details while allowing verification
#[contracttype]
#[derive(Clone, Debug)]
pub struct PrivateTransaction {
    /// Hash of sender (or commitment)
    pub sender_hash: Bytes,
    /// Hash of receiver (or commitment)
    pub receiver_hash: Bytes,
    /// Commitment to the amount
    pub amount_commitment: Bytes,
    /// Commitment to sender's balance after transaction
    pub sender_new_balance_commitment: Bytes,
    /// Commitment to receiver's balance after transaction
    pub receiver_new_balance_commitment: Bytes,
    /// Proof that the transaction is valid
    pub validity_proof: ZKProof,
    /// Range proof for the amount
    pub amount_range_proof: RangeProof,
    /// Timestamp for compliance
    pub timestamp: u64,
    /// Unique transaction ID for audit trail
    pub transaction_id: Bytes,
}

/// Proof Verification Result containing details about verification
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofVerificationResult {
    Valid,
    Invalid,
    MalformedProof,
    ExpiredProof,
}

/// Parameters for circuit computations
#[contracttype]
#[derive(Clone, Debug)]
pub struct CircuitParameters {
    /// Domain separator for soundness
    pub domain: Bytes,
    /// Generator point for commitments
    pub generator_g: Bytes,
    /// Alternative generator for blinding
    pub generator_h: Bytes,
    /// Hash function identifier
    pub hash_function: u32,
}

impl Default for CircuitParameters {
    fn default() -> Self {
        use soroban_sdk::{Bytes, Env};
        let env = Env::default();
        CircuitParameters {
            domain: Bytes::from_array(&env, b"swaptrade_zkp_v1"),
            generator_g: Bytes::from_array(&env, &[0u8; 32]),
            generator_h: Bytes::from_array(&env, &[1u8; 32]),
            hash_function: 0, // 0 = SHA-256
        }
    }
}

/// Audit Log Entry for compliance and transparency
#[contracttype]
#[derive(Clone, Debug)]
pub struct AuditLogEntry {
    /// Transaction ID being logged
    pub transaction_id: Bytes,
    /// Type of audit event
    pub event_type: AuditEventType,
    /// Timestamp of event
    pub timestamp: u64,
    /// Verification result
    pub verification_result: ProofVerificationResult,
    /// Hash of the transaction for verification
    pub transaction_hash: Bytes,
}

/// Types of audit events
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditEventType {
    ProofGenerated,
    ProofVerified,
    ProofFailed,
    TransactionExecuted,
    ComplianceCheck,
}

/// Performance Metrics for ZKP operations
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProofMetrics {
    /// Time to generate proof (milliseconds)
    pub generation_time_ms: u64,
    /// Time to verify proof (milliseconds)
    pub verification_time_ms: u64,
    /// Size of proof in bytes
    pub proof_size_bytes: u32,
    /// Gas used for verification
    pub verification_gas: u64,
}

/// Configuration for the ZKP system
#[contracttype]
#[derive(Clone, Debug)]
pub struct ZKPConfig {
    /// Maximum amount for transactions (as commitment)
    pub max_amount: i128,
    /// Minimum amount for transactions
    pub min_amount: i128,
    /// Proof TTL in seconds
    pub proof_ttl_seconds: u64,
    /// Maximum proofs per block
    pub max_proofs_per_block: u32,
    /// Enabled proof schemes
    pub enabled_schemes: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_scheme_values() {
        assert_ne!(ProofScheme::Bulletproof, ProofScheme::ZkSnark);
        assert_ne!(ProofScheme::Bulletproof, ProofScheme::SimplifiedProof);
        assert_ne!(ProofScheme::ZkSnark, ProofScheme::SimplifiedProof);
    }

    #[test]
    fn test_audit_event_type_values() {
        assert_ne!(
            AuditEventType::ProofGenerated,
            AuditEventType::ProofVerified
        );
        assert_ne!(
            AuditEventType::ProofVerified,
            AuditEventType::TransactionExecuted
        );
    }
}