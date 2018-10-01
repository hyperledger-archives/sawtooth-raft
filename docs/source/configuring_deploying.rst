***************************************
Configuring and Deploying Sawtooth Raft
***************************************

This guide assumes previous experience with creating and deploying Sawtooth
networks; if you are unfamiliar with Sawtooth, please see the `Application
Developer's Guide`_ and `System Administrator's Guide`_ in the Sawtooth Core
documentation.

.. _Application Developer's Guide: https://sawtooth.hyperledger.org/docs/core/nightly/master/app_developers_guide.html
.. _System Administrator's Guide: https://sawtooth.hyperledger.org/docs/core/nightly/master/sysadmin_guide.html

This guide also requires a basic understanding of the Raft consensus algorithm;
if you are not familiar with Raft, please see the `Raft`_ webpage.

.. _Raft: https://raft.github.io/


.. _validator-peering-requirements-label:

Validator Peering Requirements
==============================

Sawtooth Raft requires a fully-peered network where all nodes are able to
communicate with all other nodes. This is necessary to establish a leader
through the election process, as well as to maintain leadership through regular
heartbeat messages. If a Raft network is not fully-peered, leadership election
may fail due to some nodes being unable to communicate with one another.

In order to achieve a fully-peered network, the list of peers that comprise the
network will need to be known and maintained at all times. This means that Raft
networks do not support open membership; an administrator must explicitly add or
remove nodes to change membership (see `Changing Network Membership`_ for more
details).

To setup a fully-peered network, all validators must be started with the
following two options:

* ``--peering static`` - Informs the validator to connect with all of the
  specified peers.
* ``--peers {peers_list}`` - Provides the list of peers for the validator to
  connect to, where `{peers_list}` is the list of public endpoints for all other
  previously started validators.

For example, if there are two validators already running with public endpoints
``203.0.113.1:8800`` and ``203.0.113.2:8800``, a new validator would be started
with the options ``--peering static --peers
tcp://203.0.113.1:8800,tcp://203.0.113.2:8800``.

Since peering is bi-directional, when a new validator connects with existing
validators, the existing validators will be fully peered as well. For example,
to startup a 3 node network where the nodes have endpoints ``alpha``, ``beta``,
and ``gamma``, you would do the following:

1. Startup the alpha validator with no peers specified (but ``--peering static``
   should still be used)
2. Startup the beta validator with ``--peers tcp://alpha:8800``
3. Startup the gamma validator with ``--peers tcp://alpha:8800,tcp://beta:8800``


On-Chain Settings
=================

When starting a fresh network with the Sawtooth Raft consensus engine, the
required settings below (and the desired optional settings) should be set in the
genesis block.


Required Settings
-----------------

* ``sawtooth.consensus.algorithm`` - Tells the network which consensus engine to
  use. Must be set to ``raft``.
* ``sawtooth.consensus.raft.peers`` - A JSON list of public keys of all
  validators in the network, formatted as hex strings.

The ``sawtooth.consensus.raft.peers`` setting is extremely important, since the
Raft engine uses it as its source-of-truth for which nodes to communicate with.
When a Raft engine starts initially, it checks the setting to determine which
nodes are in the network. This setting is also used to add or remove nodes from
the Raft network as covered in the `Changing Network Membership`_ section. As an
example, the value of ``sawtooth.consensus.raft.peers`` would be formatted like
this:

.. code-block:: json

  [
    "dc26a7099e81bb02869cc8ae57da030fbe4cf276b38ab37d2cc815fec63a14ab",
    "df8e8388ced559bd35c2b05199ca9f8fbebb420979715003355dcb7363016c1d"
  ]


Optional Settings
-----------------

* ``sawtooth.consensus.raft.heartbeat_tick`` - Determines the interval at which
  the leader sends heartbeat messages to all other nodes. Default is ``2``.
* ``sawtooth.consensus.raft.election_tick`` - Determines the timeout after which
  nodes can start new elections if no messages have been received from the
  leader. Default is ``20``.
* ``sawtooth.consensus.raft.period`` - Determines the length of time (in
  milliseconds) that the consensus engine will wait between initializing and
  finalizing a block. Default is ``3000``.

These optional settings should only be set `before` any Raft engines have
started up; if they are changed on a running network and a new node is started,
these values will be different on the new engine than the rest of the network,
which could result in unpredictable behavior.


Starting a Raft Engine
======================


Configuring the State Storage Directory
---------------------------------------

The Raft engine stores its state on the file system; by default, it uses the
``/var/lib/sawtooth-raft`` directory, but this can be changed by setting the
``SAWTOOTH_RAFT_HOME`` environment variable to the desired path.


Logging Configuration
---------------------

Sawtooth Raft logs to the console by default, but supports
configurable logging through the `log4rs`_ and `log4rs-syslog`_ libraries. The
`sawtooth-raft repository`_ provides an example logging configuration file that
demonstrates how to configure Raft to use syslog (``logging/syslog.yaml``). For
more examples of logging configuration files, please see the
`log4rs documentation`_.

.. _log4rs: https://github.com/sfackler/log4rs
.. _log4rs-syslog: https://github.com/im-0/log4rs-syslog
.. _sawtooth-raft repository: https://github.com/hyperledger/sawtooth-raft
.. _log4rs documentation: https://docs.rs/log4rs/0.8.0/log4rs/

The Raft engine provides the ``-L`` option for specifying a logging
configuration YAML file. For example, to start Raft with the provided syslog
configuration, you would provide the option ``-L logging/syslog.yaml``.

Sawtooth Raft also supports configurable logging verbosity at run-time. By
default, it logs with the ``WARN`` logging level (or the logging level specified
in the configuration file), but this can be changed by providing one or more
verbosity flags on startup:

- ``-v`` - ``INFO`` logging level
- ``-vv`` - ``DEBUG`` logging level
- ``-vvv`` - ``TRACE`` logging level


Connecting to the Validator
---------------------------

When a Sawtooth Raft engine is started, it must be connected to the validator,
which can be done using the ``--connect`` command line option. For example, if
the validator is running on host ``203.0.113.0`` and is using the default
consensus port ``5050``, then the Raft engine should be started with ``--connect
tcp://127.0.0.1:5050``. If this option is not specified, Sawtooth Raft will
attempt to connect with the default validator address: ``tcp://localhost:5050``.


Verifying Raft Is running
-------------------------

This section assumes that Raft is being run with logging level ``INFO``.

When the Sawtooth Raft process starts initially, you will see a message that
indicates the version of the Raft engine and the validator endpoint that it is
attempting to connect to:

.. code-block:: console

  INFO  | sawtooth_raft:84     | Sawtooth Raft Engine (X.Y.Z)
  INFO  | sawtooth_raft:90     | Raft Node connecting to 'tcp://validator:5050'

This indicates that the Raft process has started; however, it does not indicate
that the Raft engine itself is running. The Raft engine waits until the genesis
block has been received and committed by the validator before running. Once the
genesis block has been committed, you will see a message in the validator's logs
indicating that the Raft engine has been registered:

.. code-block:: console

  Consensus engine registered: sawtooth-raft X.Y.Z

If you do not see the message above in the validator logs, make sure that the
Raft engine is properly connecting to the validator (see `Connecting to the
Validator`_) and that the validator has committed the genesis block with the
required Raft settings (see `Creating the Genesis Block`_ in the Sawtooth Core
documentation).

.. _Creating the Genesis Block: https://sawtooth.hyperledger.org/docs/core/nightly/master/sysadmin_guide/creating_genesis_block.html

Once the consensus engine is running and connected to the validator, you will
see a message in the Raft log that displays the configuration values that are
being used by the Raft engine, similar to this one:

.. code-block:: console

  INFO  | sawtooth_raft::engin | Raft Engine Config Loaded: RaftEngineConfig { peers: [026c49c05b153ca92e2fae01fea85663ae397eb435cc7744907edfe839e84fb288], period: 3s, raft: { election_tick: 20, heartbeat_tick: 2, applied: 0 }, storage: cached storage: file-system backed persistent storage }

You should also see a group of messages that indicate if the node has been
elected leader, similar to this:

.. code-block:: console

  INFO  | raft::raft:833       | [15456778813275318575] became follower at term 0
  INFO  | raft::raft:433       | [15456778813275318575] newRaft [peers: [15456778813275318575], term: 0, commit: 0, applied: 0, last_index: 0, last_term: 0]
  INFO  | raft::raft:833       | [15456778813275318575] became follower at term 1
  INFO  | raft::raft:1120      | [15456778813275318575] is starting a new election at term 1
  INFO  | raft::raft:848       | [15456778813275318575] became candidate at term 2
  INFO  | raft::raft:950       | [15456778813275318575] received MsgRequestVoteResponse from 15456778813275318575 at term 2
  INFO  | raft::raft:891       | [15456778813275318575] became leader at term 2

At this point, the Raft engine is running and ready to handle blocks.


Starting a Multi-Node Raft Network
==================================

The `sawtooth-raft repository`_ provides a set of Docker Compose files in the
``adhoc`` directory that allow one to quickly and easily setup a Raft network
using Docker. The compose files in this directory are designed to make manual,
ad-hoc deployments and testing of Sawtooth Raft networks simpler.


Starting the Network
--------------------

Make sure you have all the Docker images you need:

- sawtooth-validator (>= v1.1)
- sawtooth-rest-api
- sawtooth-intkey-tp-python
- sawtooth-intkey-workload
- sawtooth-settings-tp

To build these from `sawtooth-core`_, run the following:

.. _sawtooth-core: https://github.com/hyperledger/sawtooth-core
.. code-block:: console

  $ docker-compose -f docker-compose-installed.yaml build \
      validator \
      rest-api \
      intkey-tp-python \
      intkey-workload \
      settings-tp


Starting the Admin Service
~~~~~~~~~~~~~~~~~~~~~~~~~~

This compose file creates the shared network and volume required for the
validator network to communicate.

.. code-block:: console

  $ docker-compose -f admin.yaml up -d


Starting the Nodes
~~~~~~~~~~~~~~~~~~

If N is the number of nodes you want to create, startup N-1 nodes with the
``node.yaml`` compose file. For each node, you will need to select a unique name
and provide this with the ``-p`` option. Example:

.. code-block:: console

  $ docker-compose -p alpha -f node.yaml up

Note that you can include the ``-d`` flag to create all nodes from a single
terminal.

For the last node, do the same as above, but also set the ``GENESIS`` environment
variable. Example:

.. code-block:: console

  $ GENESIS=1 docker-compose -p genesis -f node.yaml up

Note that each time a node is started, it adds itself to the list of nodes that
all new nodes will connect to. This means that if you mess something up starting
a node, you may need to start over. To do this, use ``docker-compose down`` for
each node you started, passing the appropriate ``-p`` value; then do
``docker-compose -f admin.yaml down``.


Stopping and Restarting Nodes On a Network
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

To stop a node on a network, you need to stop the containers for that node
**without removing them**. If you remove the containers, then that node is gone
for good. To avoid for this, do **NOT** use ``docker-compose down``.

If you are attached to the containers, you can press CTRL+C to stop the node's
containers.

If you are not attached to the containers, use the command ``docker-compose
stop`` with the appropriate ``-p`` flag.

After you have stopped a node, you can use ``docker-compose start`` with the
appropriate ``-p`` flag to restart the stopped node.


Verifying the Network Is Ready
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

When the network is setup correctly, Raft will elect a node as leader. If all
nodes are peered properly, you should also see a group of messages in the logs
of each Raft engine that indicate if the node has been elected leader or if it
has voted for another node as leader. On a leader node, the logs will look
similar to this:

.. code-block:: console

  INFO  | raft::raft:833       | [12797589408118497989] became follower at term 0
  INFO  | raft::raft:433       | [12797589408118497989] newRaft [peers: [9142281103993713288, 12797589408118497989], term: 0, commit: 0, applied: 0, last_index: 0, last_term: 0]
  INFO  | raft::raft:833       | [12797589408118497989] became follower at term 1
  INFO  | raft::raft:1120      | [12797589408118497989] is starting a new election at term 1
  INFO  | raft::raft:848       | [12797589408118497989] became candidate at term 2
  INFO  | raft::raft:950       | [12797589408118497989] received MsgRequestVoteResponse from 12797589408118497989 at term 2
  INFO  | raft::raft:927       | [12797589408118497989] [logterm: 1, index: 2] sent MsgRequestVote request to 9142281103993713288 at term 2
  INFO  | raft::raft:950       | [12797589408118497989] received MsgRequestVoteResponse from 9142281103993713288 at term 2
  INFO  | raft::raft:1648      | [12797589408118497989] [quorum:2] has received 2 MsgRequestVoteResponse votes and 0 vote rejections
  INFO  | raft::raft:891       | [12797589408118497989] became leader at term 2

On a node that becomes a follower and votes for another node, the logs will look
like this:

.. code-block:: console

  INFO  | raft::raft:833       | [9142281103993713288] became follower at term 0
  INFO  | raft::raft:433       | [9142281103993713288] newRaft [peers: [9142281103993713288, 12797589408118497989], term: 0, commit: 0, applied: 0, last_index: 0, last_term: 0]
  INFO  | raft::raft:833       | [9142281103993713288] became follower at term 1
  INFO  | raft::raft:1014      | [9142281103993713288] [term: 1] received a MsgRequestVote message with higher term from 12797589408118497989 [term: 2]
  INFO  | raft::raft:833       | [9142281103993713288] became follower at term 2
  INFO  | raft::raft:1181      | [9142281103993713288] [logterm: 1, index: 2, vote: 0] cast MsgRequestVote for 12797589408118497989 [logterm: 1, index: 2] at term 2

This indicates that a leader has been elected and that the network is ready.


Applying Workload On a Network
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A workload generator can be started against the network using the
``workload.yaml`` compose file. To change the rate from the default, set the
``RATE`` environment variable prior to running the compose command.

For example, to start a workload of 2 TPS against the network, do:

.. code-block:: console

  $ RATE=2 docker-compose -f workload.yaml up

To stop the workload, simply do:

.. code-block:: console

  $ docker-compose -f workload.yaml down


Changing Network Membership
===========================

Even though membership of a Sawtooth Raft network is not open, it does support
changes. Membership is changed by updating the ``sawtooth.consensus.raft.peers``
on-chain setting to add a new node or remove an existing node. The leader
monitors this setting; when it detects that a new key is present (adding a node)
or that a key has been deleted (removing a node), it stops publishing blocks
and proposes the configuration change to the network.

Due to limitations in the raft-rs library, only one node may be added or removed
at a time. This means that the updated value of
``sawtooth.consensus.raft.peers`` may have a single key removed **OR** it may
have a single key added, but not both. If the leader detects that more than one
node's membership has changed, it will report an error and shutdown.


Adding a Node
-------------

The new Sawtooth Raft engine should be started before the update to the on-chain
setting is made. Once the setting is updated to include the public key of the
new node and the network has been made aware of the change, the leader will
begin to send messages to the new node and bring it into consensus with the rest
of the network.


Removing a Node
---------------

The on-chain setting should be updated before the node is shutdown. When the
setting has been updated to no longer include the old node's public key, the
leader will stop sending messages to the node and ignore any messages from it.
Once this happens, the node can be shutdown safely since it is no longer
participating in the network.
