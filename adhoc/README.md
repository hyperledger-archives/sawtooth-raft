<!--
Copyright 2018 Intel Corporation

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
------------------------------------------------------------------------------
-->

# Ad-Hoc Sawtooth Raft Networks

The compose files in this directory are designed to make manual, ad-hoc testing
of Sawtooth Raft networks simpler.

## Starting up a network

1. Make sure you have all the images you need.

    - sawtooth-validator (>= v1.1)
    - sawtooth-rest-api
    - sawtooth-intkey-tp-python
    - sawtooth-settings-tp

   To build these from sawtooth-core, do:

    ```
    docker-compose -f docker-compose-installed.yaml build \
      validator \
      rest-api \
      intkey-tp-python \
      settings-tp
    ```

2. Start the admin service. This compose file creates the shared network and
   volume required for the validator network to communicate.

    ```
    docker-compose -f admin.yaml up -d
    ```

3. If N is the number of validators you want to create, startup N-1 validators
   with the node.yaml compose file. For each validator you will need to select
   a unique name and provide this with the `-p` flag.

   Example:

    `docker-compose -p alpha -f node.yaml up`

   Note that you can include the `-d` flag to create all validators from a
   single terminal.

4. For the last validator, do the same as above, but also set the `GENESIS`
   environment variable.

   Example:

    `GENESIS=1 docker-compose -p genesis -f node.yaml up`

Note that each time a validator is started, it adds itself to the list of
validators that all new validators will connect to. This means that if you mess
something up starting a validator, you may need to start over. To do this, use
`docker-compose down` for each validator you started, passing the appropriate
`-p` value and then do `docker-compose -f admin.yaml down`.

## Stopping and restarting nodes on a network

To stop a node on a network, you need to stop the containers for that node
**without removing them***. If you remove the containers, then that node is
gone for good. The command to avoid for this is `docker-compose down`.

If you are attached to the containers, you can press CTRL+C to
stop the node's containers.

If you are not attached to the containers, use the command `docker-compose
stop` with the appropriate `-p` flag.

After you have stopped a node, you can use `docker-compose start` with the
appropriate `-p` flag to restart the stopped node.
