************
Introduction
************

Sawtooth Raft is a consensus engine for Hyperledger Sawtooth based on the crash
fault tolerant consensus algorithm `Raft`_.
Specifically, it is built on top of the Rust implementation of Raft used by
TiKV, `raft-rs`_.

.. _Raft: http://raft.github.io/
.. _raft-rs: https://github.com/pingcap/raft-rs

Sawtooth Raft differs from the PoET SGX and devmode consensus engines in a few
major ways:

* **Raft is a leader-based consensus algorithm** - The Raft algorithm defines
  an election process whereby a leader is established and recognized by all
  followers. This means that only one node—the leader—publishes blocks, which
  are then validated and agreed on by the other nodes in the network—the
  followers.
* **Raft is non-forking** - No forks arise on a Raft network because only one
  node can publish blocks. This means that no fork resolution needs to be done,
  which reduces the overhead associated with block management and lends itself
  to increased performance.
* **Raft membership is fixed** - Raft requires that a majority of nodes agree on
  all progress that is made. To ensure safety by preventing invalid node
  configurations, Sawtooth Raft does not support open membership. The population
  of nodes is fixed unless changed by an administrator.
* **Raft networks should be small** - Raft requires exchanging messages between
  all nodes to reach consensus for each block, and the leader must wait for a
  majority of nodes to agree on a new entry before it can be committed. The
  number of messages that need to be exchanged increases exponentially with the
  size of the network, which make large networks impractical.
* **Raft is not Byzantine fault tolerant** - The Raft algorithm was designed to
  be crash fault tolerant—it can continue to make progress as long as a majority
  of its nodes are available. However, Raft only guarantees safety and
  availability under non-Byzantine conditions, which makes it ill-suited for
  networks that require Byzantine fault tolerance.

The Sawtooth Raft consensus engine may be right for you if your Sawtooth
deployment will:

1. Consist of a small number of nodes (roughly 1 to 10)
2. Have a mostly fixed membership
3. Not require Byzantine fault tolerance


Join the Sawtooth Community
===========================

Sawtooth is an open source project under the Hyperledger umbrella. We welcome
working with individuals and companies interested in advancing distributed
ledger technology. Please see :doc:`/community` for ways to become a part of
the Sawtooth community.


Acknowledgements
================

This project uses software developed by the OpenSSL Project for use in the
OpenSSL Toolkit (http://www.openssl.org/).

This project relies on other third-party components. For details, see the
LICENSE and NOTICES files in the `sawtooth-raft repository
<https://github.com/hyperledger/sawtooth-raft>`_.

.. Licensed under Creative Commons Attribution 4.0 International License
.. https://creativecommons.org/licenses/by/4.0/
