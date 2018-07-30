# Sawtooth Raft

Sawtooth Raft is a consensus engine for Hyperledger Sawtooth based on the crash
fault tolerant consensus algorithm [Raft](http://raft.github.io/).
Specifically, it is built on top of the Rust implementation of Raft used by
TiKV, [raft-rs](https://github.com/pingcap/raft-rs).

Currently, Sawtooth Raft is in the prototype phase of development and there is
additional work to be done in order to make it production worthy.

## Deploying a Sawtooth Network with Raft

Using the Sawtooth Raft engine with Sawtooth requires the following:

1. Configure Sawtooth Raft by specifying the required on-chain settings
2. Configure the Sawtooth network so that all validators can communicate
3. Start a Raft engine for every validator and connect it to the validator

Sawtooth deployments using the Sawtooth Raft consensus engine require nodes to
be fully-connected in order to function correctly. As a result, deployments
should use a small number of nodes with a relatively fixed membership. (Adding
and removing nodes is not currently supported, although we intend to add this
feature in the future).

### Configure the Sawtooth Network

When starting validators, they should be configured to use static peering using
the `--peering static` flag and each node should specify all other nodes as
their peers using the `--peers` flag.

## On-Chain Settings

The following on-chain settings configure Sawtooth Raft. The required settings
must be specified in state prior to using Raft consensus. When starting a new
network, they should be set in the genesis block. All settings are prefixed
with `sawtooth.consensus.raft`.

The only required setting is `sawtooth.consensus.raft.peers`. It must contain
a JSON list of each node's public key.

### Required Settings

| key | value |
| --- | --- |
| peers | JSON - Vec<PeerId> |

### Optional Settings

| key | value | default |
| --- | --- | --- |
| heartbeat_tick | u64 | 150 |
| election_tick | u64 | 1500 |
| period | u64 (ms) | 3 |

## Future Improvements

[ ] Stability Improvements

    Handle all updates :invalid block, peer connected, and peer disconnected.
    Only bother checking blocks in Update::BlockNew that we expected to get
    based on consensus (ie., don't check unexpected blocks).

[ ] Persistent Storage

    Replace the memory-backed storage implementation with a persistent storage
    implementation. This eliminates the need for nodes to rebuild logs from
    scratch on restart.

[ ] Configuration Changes

    Check the `sawtooth.consensus.raft.peers` settings key after every block
    commit and, if it has changed, propose a configuration change (adding or
    removing nodes).

[ ] Block Publishing Optimizations

    The current implementation alternates between waiting for a block to be
    built and waiting for a block to commit. This is simple to implement, but
    inefficient. As an optimization, building blocks could be started
    optimistically after a newly produced block has been validated so that a
    new block can be published as soon as the previous block commits.

[x] Use Public Keys as Raft IDs

    Translate Sawtooth Peer IDs directly to Raft IDs to simplify configuration.
    Eliminates the need to specify the setting in the on-chain setting.

[ ] Implement fully-configurable System Logging

    Improve logging configuration support for the engine to include using
    syslog to log to a remote address.

[ ] Write and Publish Community Documentation

    Improve existing documentation for broad community use and integrate
    documentation into existing community documentation.
