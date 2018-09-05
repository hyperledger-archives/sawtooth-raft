************
Introduction
************

Sawtooth Raft is a consensus engine for Hyperledger Sawtooth based on the crash
fault tolerant consensus algorithm `Raft`_.
Specifically, it is built on top of the Rust implementation of Raft used by
TiKV, `raft-rs`_.

.. _Raft: http://raft.github.io/
.. _raft-rs: https://github.com/pingcap/raft-rs

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
