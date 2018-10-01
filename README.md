# Sawtooth Raft

Sawtooth Raft is a consensus engine for Hyperledger Sawtooth based on the crash
fault tolerant consensus algorithm [Raft](http://raft.github.io/).
Specifically, it is built on top of the Rust implementation of Raft used by
TiKV, [raft-rs](https://github.com/pingcap/raft-rs).

Currently, Sawtooth Raft is in the prototype phase of development and there is
additional work to be done in order to make it production worthy.

Documentation
-------------

Documentation for how to use Sawtooth Raft is available here:
https://sawtooth.hyperledger.org/docs/raft/nightly/master/

License
-------

Hyperledger Sawtooth software is licensed under the [Apache License Version 2.0](LICENSE) software license.

Hyperledger Sawtooth documentation in the [docs](docs) subdirectory is licensed under
a Creative Commons Attribution 4.0 International License.  You may obtain a copy of the
license at: http://creativecommons.org/licenses/by/4.0/.
