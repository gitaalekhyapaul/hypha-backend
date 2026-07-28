use sp1_core_executor::{
    GAS_TRACE_CHUNK_THRESHOLD, MINIMAL_TRACE_CHUNK_THRESHOLD, SP1CoreOpts, ShardingThreshold,
};

pub const LOCAL_PROVER_MEMORY_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
pub const LOCAL_PROVER_TRACE_CHUNK_SLOTS: usize = 1;
pub const LOCAL_PROVER_GAS_TRACE_CHUNK_SLOTS: usize = 1;
pub const LOCAL_PROVER_SHARD_SIZE: usize = 1 << 20;
pub const LOCAL_PROVER_ELEMENT_THRESHOLD: u64 = (1 << 28) + (1 << 27);
pub const LOCAL_PROVER_HEIGHT_THRESHOLD: u64 = 1 << 22;

pub fn local_core_options() -> SP1CoreOpts {
    SP1CoreOpts {
        minimal_trace_chunk_threshold: MINIMAL_TRACE_CHUNK_THRESHOLD,
        gas_trace_chunk_threshold: GAS_TRACE_CHUNK_THRESHOLD,
        trace_chunk_slots: LOCAL_PROVER_TRACE_CHUNK_SLOTS,
        gas_trace_chunk_slots: LOCAL_PROVER_GAS_TRACE_CHUNK_SLOTS,
        memory_limit: LOCAL_PROVER_MEMORY_LIMIT,
        shard_size: LOCAL_PROVER_SHARD_SIZE,
        sharding_threshold: ShardingThreshold {
            element_threshold: LOCAL_PROVER_ELEMENT_THRESHOLD,
            height_threshold: LOCAL_PROVER_HEIGHT_THRESHOLD,
        },
        ..SP1CoreOpts::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_profile_overrides_every_environment_readable_core_option() {
        let options = local_core_options();
        assert_eq!(
            options.minimal_trace_chunk_threshold,
            MINIMAL_TRACE_CHUNK_THRESHOLD
        );
        assert_eq!(options.gas_trace_chunk_threshold, GAS_TRACE_CHUNK_THRESHOLD);
        assert_eq!(options.trace_chunk_slots, LOCAL_PROVER_TRACE_CHUNK_SLOTS);
        assert_eq!(
            options.gas_trace_chunk_slots,
            LOCAL_PROVER_GAS_TRACE_CHUNK_SLOTS
        );
        assert_eq!(options.memory_limit, LOCAL_PROVER_MEMORY_LIMIT);
        assert_eq!(options.shard_size, LOCAL_PROVER_SHARD_SIZE);
        assert_eq!(
            options.sharding_threshold,
            ShardingThreshold {
                element_threshold: LOCAL_PROVER_ELEMENT_THRESHOLD,
                height_threshold: LOCAL_PROVER_HEIGHT_THRESHOLD,
            }
        );
    }
}
