#!/bin/bash
#
# Copyright 2018 Intel Corporation
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

### This script tests dynamic membership by starting a Raft network with 3
### nodes, adding a 4th node, applying a workload, and checking that all 4 nodes
### reach block 5.

echo "Starting initial network"
docker-compose -f ../adhoc/admin.yaml up -d
docker-compose -p alpha -f ../adhoc/node.yaml up -d
docker-compose -p beta -f ../adhoc/node.yaml up -d
GENESIS=1 docker-compose -p gamma -f ../adhoc/node.yaml up -d

ADMIN=$(docker ps | grep admin | awk '{print $1;}')

echo "Gathering list of initial keys and REST APIs"
INIT_KEYS=($(docker exec $ADMIN bash -c '\
  cd /shared_data/validators && paste $(ls -1) -d , | sed s/,/\ /g'))
echo "Initial keys:" ${INIT_KEYS[*]}
INIT_APIS=($(docker exec $ADMIN bash -c 'cd /shared_data/rest_apis && ls -d *'))
echo "Initial APIs:" ${INIT_APIS[*]}

echo "Waiting until network has started"
docker exec $ADMIN bash -c 'while true; do \
  API=$(ls /shared_data/rest_apis | head -n 1); \
  BLOCK_LIST=$(sawtooth block list --url "http://$API:8008" 2>&1); \
  if [[ $BLOCK_LIST == *"BLOCK_ID"* ]]; then \
    echo "Network ready" && break; \
  else \
    echo "Still waiting..." && sleep 0.5; \
  fi; done;'

echo "Starting new node"
docker-compose -p delta -f ../adhoc/node.yaml up -d

echo "Waiting until new node is ready"
docker exec $ADMIN bash -c 'while true; do \
  KEYS=($(cd /shared_data/validators && paste $(ls -1) -d \ )); \
  if [[ "${#KEYS[@]}" -eq 4 ]]; then \
    echo "New node ready" && break; \
  else \
    echo "Still waiting..." && sleep 0.5; \
  fi; done;'

echo "Adding new node to Raft network"
docker exec $ADMIN bash -c '\
  API=$(ls /shared_data/rest_apis | head -n 1); \
  NEW_PEERS=$(cd /shared_data/validators && paste $(ls -1) -d , \
    | sed s/,/\\\",\\\"/g); \
  SETTING_PEERS=($(sawtooth settings list --url "http://$API:8008" \
    --filter "sawtooth.consensus.raft.peers" --format csv | sed -n 2p | \
    sed "s/\"\",\"\"/\ /g")); \
  until [[ "${#SETTING_PEERS[@]}" -eq 4 ]]; do \
    echo "Attempting to set sawtooth.consensus.raft.peers..."; \
    # Try to update setting \
    sawset proposal create -k /shared_data/keys/settings.priv \
      --url "http://$API:8008" sawtooth.consensus.raft.peers=[\"$NEW_PEERS\"]; \
    # Wait and see if setting has been updated \
    sleep 5; \
    SETTING_PEERS=($(sawtooth settings list --url "http://$API:8008" \
      --filter "sawtooth.consensus.raft.peers" --format csv | sed -n 2p | \
      sed "s/\"\",\"\"/\ /g")); \
  done;'

NEW_KEYS=($(docker exec $ADMIN bash -c '\
  cd /shared_data/validators && paste $(ls -1) -d , | sed s/,/\ /g'))
NEW_APIS=($(docker exec $ADMIN bash -c 'cd /shared_data/rest_apis && ls -d *'))
NEW_KEY=($(comm -3 <(printf '%s\n' "${INIT_KEYS[@]}" | LC_ALL=C sort) \
  <(printf '%s\n' "${NEW_KEYS[@]}" | LC_ALL=C sort)))
NEW_API=($(comm -3 <(printf '%s\n' "${INIT_APIS[@]}" | LC_ALL=C sort) \
  <(printf '%s\n' "${NEW_APIS[@]}" | LC_ALL=C sort)))
echo "New keys:" ${NEW_KEYS[*]}
echo "New APIs:" ${NEW_APIS[*]}
echo "New node key:" $NEW_KEY
echo "New node API:" $NEW_API

echo "Waiting for new node to receive block"
docker exec -e API=$NEW_API $ADMIN bash -c 'while true; do \
  BLOCK_LIST=$(sawtooth block list --url "http://$API:8008" 2>&1); \
  if [[ $BLOCK_LIST == *"BLOCK_ID"* ]]; then \
    echo "New node has received block" && break; \
  else \
    echo "Still waiting..." && sleep 0.5; \
  fi; done;'

echo "Starting workload"
RATE=5 docker-compose -f ../adhoc/workload.yaml up -d

echo "Waiting for all nodes to reach block 5"
docker exec $ADMIN bash -c '\
  APIS=$(cd /shared_data/rest_apis && ls -d *); \
  NODES_ON_5=0; \
  until [ "$NODES_ON_5" -eq 4 ]; do \
    NODES_ON_5=0; \
    sleep 5; \
    for api in $APIS; do \
      BLOCK_LIST=$(sawtooth block list --url "http://$api:8008" \
        | cut -f 1 -d " "); \
      if [[ $BLOCK_LIST == *"5"* ]]; then \
        echo "API $api is on block 5" && ((NODES_ON_5++)); \
      else \
        echo "API $api is not yet on block 5"; \
      fi; \
    done; \
  done;'
echo "All nodes have reached block 5!"

echo "Done testing; shutting down all containers"
docker-compose -f ../adhoc/workload.yaml down && \
docker-compose -p alpha -f ../adhoc/node.yaml down && \
docker-compose -p beta -f ../adhoc/node.yaml down && \
docker-compose -p gamma -f ../adhoc/node.yaml down && \
docker-compose -p delta -f ../adhoc/node.yaml down && \
docker-compose -f ../adhoc/admin.yaml down
