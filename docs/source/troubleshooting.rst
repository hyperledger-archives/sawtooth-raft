*********************************
Troubleshooting and Common Issues
*********************************

Below is a list of common issues that are seen when attempting to start and run
a Sawtooth Raft network. If you encounter any problems, please consult this list
to see if your issue is discussed. If you are still having problems, find out
how to get help in the :doc:`/community/join_the_discussion` section.


No Leader Is Elected
====================

When a Sawtooth Raft network is unable to elect a leader, one or more nodes will
continuously start new elections. The logs will probably look similar to this:

.. code-block:: console

  INFO  | raft::raft:833       | [10283357333226787554] became follower at term 1
  INFO  | raft::raft:1120      | [10283357333226787554] is starting a new election at term 1
  INFO  | raft::raft:848       | [10283357333226787554] became candidate at term 2
  INFO  | raft::raft:950       | [10283357333226787554] received MsgRequestVoteResponse from 10283357333226787554 at term 2
  INFO  | raft::raft:927       | [10283357333226787554] [logterm: 1, index: 2] sent MsgRequestVote request to 6831727538046783822 at term 2
  INFO  | raft::raft:1120      | [10283357333226787554] is starting a new election at term 2
  INFO  | raft::raft:848       | [10283357333226787554] became candidate at term 3
  INFO  | raft::raft:950       | [10283357333226787554] received MsgRequestVoteResponse from 10283357333226787554 at term 3
  INFO  | raft::raft:927       | [10283357333226787554] [logterm: 1, index: 2] sent MsgRequestVote request to 6831727538046783822 at term 3
  INFO  | raft::raft:1120      | [10283357333226787554] is starting a new election at term 3
  INFO  | raft::raft:848       | [10283357333226787554] became candidate at term 4
  INFO  | raft::raft:950       | [10283357333226787554] received MsgRequestVoteResponse from 10283357333226787554 at term 4
  INFO  | raft::raft:927       | [10283357333226787554] [logterm: 1, index: 2] sent MsgRequestVote request to 6831727538046783822 at term 4
  INFO  | raft::raft:1120      | [10283357333226787554] is starting a new election at term 4
  INFO  | raft::raft:848       | [10283357333226787554] became candidate at term 5
  INFO  | raft::raft:950       | [10283357333226787554] received MsgRequestVoteResponse from 10283357333226787554 at term 5
  INFO  | raft::raft:927       | [10283357333226787554] [logterm: 1, index: 2] sent MsgRequestVote request to 6831727538046783822 at term 5
  ...

This is usually a sign that the nodes are not peering properly. If you encounter
this problem, see `Troubleshooting Connectivity`_ for tips on how to resolve it.


Troubleshooting Connectivity
============================

If a Raft network is not setup correctly, some nodes may not be able to
communicate; this is detrimental to the functionality of the Raft algorithm, so
it is usually quite obvious when there is a connectivity issue. Common signs
that some nodes are unable to communicate with each other include:

- Failing to elect or keep a leader
- One or more nodes not making progress with the rest of the network
- Nodes repeatedly sending a message without a response

Below are a few common causes for node connectivity issues and how to resolve
them.


Validator Peering Configuration
-------------------------------

If you are experiencing network peering problems, the first thing to do is to
verify that the validators are configured to peer properly. If the validators
are not started up with static peering or the list of all existing validator
endpoints is not supplied to a validator, some nodes will not be able to
communicate. See the section on :ref:`validator-peering-requirements-label` to
ensure that you are providing the correct parameters for the validators to peer
properly.


Max Peers Exceeded
------------------

If you are running a large Raft network (around 10 nodes), you might see an
error in one or more of the validators' logs indicating that a peering request
was rejected because the validator has reached its maximum number of peers. The
message will look like this:

.. code-block:: console

  Unable to successfully peer with connection_id: 208f8143e9f50aeacf7583bc599e2e4dcd206acfbd0499de6f9b05ac49ad44fcc8a993dea62c43fdbdfe78e19e41c4efec6f7b6e0c2a9a1722b8d008bf97ad91, due to At maximum configured number of peers: 10 Rejecting peering request from tcp://03a56e9f368a:8800.

To resolve this issue, you will need to increase the peer limit for all
validators. If you are starting the validator from the command line, you can
specify the maximum number of peer connections with the
``--maximum_peer_connectivity`` option (see the `sawtooth-validator
documentation`_ for more details).

.. _sawtooth-validator documentation: https://sawtooth.hyperledger.org/docs/core/nightly/master/cli/sawtooth-validator.html


Troubleshooting ``sawtooth.consensus.raft.peers`` Setting
=========================================================

When the ``sawtooth.consensus.raft.peers`` on-chain setting is not configured
properly, the Raft network will fail to work. This may result in nodes not
communicating properly, or it may cause one or more nodes to crash with a
message like:

.. code-block:: console

  raft_1              | thread 'main' panicked at 'Peer id not valid hex: OddLength', libcore/result.rs:945:5
  raft_1              | note: Run with `RUST_BACKTRACE=1` for a backtrace.

Here are some tips for troubleshooting this setting.

List Format
-----------

The list of peers must be a valid JSON formatted list, similar to this:

.. code-block:: console

  sawtooth.consensus.raft.peers=["dc26a7099e81bb02869cc8ae57da030fbe4cf276b38ab37d2cc815fec63a14ab","df8e8388ced559bd35c2b05199ca9f8fbebb420979715003355dcb7363016c1d"]

If your list looks like the above but you are still encountering errors, try
escaping the quotes in the JSON string:

.. code-block:: console

  sawtooth.consensus.raft.peers=[\"dc26a7099e81bb02869cc8ae57da030fbe4cf276b38ab37d2cc815fec63a14ab\",\"df8e8388ced559bd35c2b05199ca9f8fbebb420979715003355dcb7363016c1d\"]
