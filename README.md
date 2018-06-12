# Sawtooth Raft

All raft engine must be started with a unique id. No id may be 0.

## On-Chain Settings

The following on-chain settings configure Sawtooth Raft. The required settings
must be specified in state prior to using Raft consensus. When starting a new
network, they should be set in the genesis block.

All settings are prefixed with `sawtooth.consensus.raft`.

### Required Settings

| key | value |
| --- | --- |
| peers | JSON - Map<u64, PeerId> |

### Optional Settings

| key | value | default |
| --- | --- | --- |
| heartbeat_tick | u64 | 150 |
| election_tick | u64 | 1500 |
| period | u64 (ms) | 3 |
