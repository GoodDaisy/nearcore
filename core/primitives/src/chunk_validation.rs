use std::collections::HashMap;

use crate::challenge::PartialState;
use crate::sharding::{ChunkHash, ReceiptProof, ShardChunkHeader};
use crate::transaction::SignedTransaction;
use borsh::{BorshDeserialize, BorshSerialize};
use near_crypto::Signature;
use near_primitives_core::hash::CryptoHash;
use near_primitives_core::types::AccountId;

/// The state witness for a chunk; proves the state transition that the
/// chunk attests to.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct ChunkStateWitness {
    /// The chunk header that this witness is for. While this is not needed
    /// to apply the state transition, it is needed for a chunk validator to
    /// produce a chunk endorsement while knowing what they are endorsing.
    pub chunk_header: ShardChunkHeader,
    /// The base state and post-state-root of the main transition where we
    /// apply transactions and receipts. Corresponds to the state transition
    /// that takes us from the pre-state-root of the last new chunk of this
    /// shard to the post-state-root of that same chunk.
    pub main_state_transition: ChunkStateTransition,
    /// For the main state transition, we apply transactions and receipts.
    /// Exactly which of them must be applied is a deterministic property
    /// based on the blockchain history this chunk is based on.
    ///
    /// The set of receipts is exactly
    ///   Filter(R, |receipt| receipt.target_shard = S), where
    ///     - R is the set of outgoing receipts included in the set of chunks C
    ///       (defined below),
    ///     - S is the shard of this chunk.
    ///
    /// The set of chunks C, from which the receipts are sourced, is defined as
    /// all new chunks included in the set of blocks B.
    ///
    /// The set of blocks B is defined as the contiguous subsequence of blocks
    /// B1 (EXCLUSIVE) to B2 (inclusive) in this chunk's chain (i.e. the linear
    /// chain that this chunk's parent block is on), where B1 is the block that
    /// contains the last new chunk of shard S before this chunk, and B1 is the
    /// block that contains the last new chunk of shard S before B2.
    ///
    /// Furthermore, the set of transactions to apply is exactly the
    /// transactions included in the chunk of shard S at B2.
    ///
    /// For the purpose of this text, a "new chunk" is defined as a chunk that
    /// is proposed by a chunk producer, not one that was copied from the
    /// previous block (commonly called a "missing chunk").
    ///
    /// This field, `source_receipt_proofs`, is a (non-strict) superset of the
    /// receipts that must be applied, along with information that allows these
    /// receipts to be verifiable against the blockchain history.
    pub source_receipt_proofs: HashMap<ChunkHash, ReceiptProof>,
    /// An overall hash of the list of receipts that should be applied. This is
    /// redundant information but is useful for diagnosing why a witness might
    /// fail. This is the hash of the borsh encoding of the Vec<Receipt> in the
    /// order that they should be applied.
    pub applied_receipts_hash: CryptoHash,
    /// The transactions to apply. These must be in the correct order in which
    /// they are to be applied.
    pub transactions: Vec<SignedTransaction>,
    /// For each missing chunk after the last new chunk of the shard, we need
    /// to carry out an implicit state transition. Mostly, this is for
    /// distributing validator rewards. This list contains one for each such
    /// chunk, in forward chronological order.
    ///
    /// After these are applied as well, we should arrive at the pre-state-root
    /// of the chunk that this witness is for.
    pub implicit_transitions: Vec<ChunkStateTransition>,
    /// Finally, we need to be able to verify that the new transitions proposed
    /// by the chunk (that this witness is for) are valid. For that, we need
    /// the transactions as well as another partial storage (based on the
    /// pre-state-root of this chunk) in order to verify that the sender
    /// accounts have appropriate balances, access keys, nonces, etc.
    pub new_transactions: Vec<SignedTransaction>,
    pub new_transactions_validation_state: PartialState,
}

/// Represents the base state and the expected post-state-root of a chunk's state
/// transition. The actual state transition itself is not included here.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct ChunkStateTransition {
    /// The block that contains the chunk; this identifies which part of the
    /// state transition we're talking about.
    pub block_hash: CryptoHash,
    /// The partial state before the state transition. This includes whatever
    /// initial state that is necessary to compute the state transition for this
    /// chunk.
    pub base_state: PartialState,
    /// The expected final state root after applying the state transition.
    /// This is redundant information, because the post state root can be
    /// derived by applying the state transition onto the base state, but
    /// this makes it easier to debug why a state witness may fail to validate.
    pub post_state_root: CryptoHash,
}

/// The endorsement of a chunk by a chunk validator. By providing this, a
/// chunk validator has verified that the chunk state witness is correct.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct ChunkEndorsement {
    pub inner: ChunkEndorsementInner,
    pub account_id: AccountId,
    pub signature: Signature,
}

/// This is the part of the chunk endorsement that is actually being signed.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct ChunkEndorsementInner {
    pub chunk_hash: ChunkHash,
    /// An arbitrary static string to make sure that this struct cannot be
    /// serialized to look identical to another serialized struct. For chunk
    /// production we are signing a chunk hash, so we need to make sure that
    /// this signature means something different.
    ///
    /// This is a messy workaround until we know what to do with NEP 483.
    signature_differentiator: String,
}

impl ChunkEndorsementInner {
    pub fn new(chunk_hash: ChunkHash) -> Self {
        Self { chunk_hash, signature_differentiator: "ChunkEndorsement".to_owned() }
    }
}

/// Stored on disk for each chunk, including missing chunks, in order to
/// produce a chunk state witness when needed.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct StoredChunkStateTransitionData {
    /// The partial state that is needed to apply the state transition,
    /// whether it is a new chunk state transition or a implicit missing chunk
    /// state transition.
    pub base_state: PartialState,
    /// If this is a new chunk state transition, the hash of the receipts that
    /// were used to apply the state transition. This is redundant information,
    /// but is used to validate against `StateChunkWitness::exact_receipts_hash`
    /// to ease debugging of why a state witness may be incorrect.
    pub receipts_hash: CryptoHash,
}
