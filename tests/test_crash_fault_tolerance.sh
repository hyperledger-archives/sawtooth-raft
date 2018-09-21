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
docker exec -e API=${INIT_APIS[0]} $ADMIN bash -c 'while true; do \
  BLOCK_LIST=$(sawtooth block list --url "http://$API:8008" 2>&1); \
  if [[ $BLOCK_LIST == *"BLOCK_ID"* ]]; then \
    echo "Network ready" && break; \
  else \
    echo "Still waiting..." && sleep 0.5; \
  fi; done;'

echo "Starting workload"
RATE=5 docker-compose -f ../adhoc/workload.yaml up -d

echo "Waiting for all nodes to reach block 5"
docker exec $ADMIN bash -c '\
  APIS=$(cd /shared_data/rest_apis && ls -d *); \
  NODES_ON_5=0; \
  until [ "$NODES_ON_5" -eq 3 ]; do \
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

echo "Shutting down alpha node"
docker stop $(docker ps | grep alpha | awk {'print $1'}) > /dev/null

echo "Waiting for alpha to fully stop"
until [ -z "$(docker ps | grep alpha)" ]; do
  echo "Still waiting..."
  sleep 5
done
echo "Alpha fully stopped"

echo "Waiting for remaining nodes to reach block 10"
docker exec $ADMIN bash -c '\
  APIS=$(cd /shared_data/rest_apis && ls -d *); \
  NODES_ON_10=0; \
  until [ "$NODES_ON_10" -eq 2 ]; do \
    NODES_ON_10=0; \
    sleep 10; \
    for api in $APIS; do \
      BLOCK_LIST=$(sawtooth block list --url "http://$api:8008" 2> /dev/null \
        | cut -f 1 -d " "); \
      if [[ $BLOCK_LIST == *"10"* ]]; then \
        echo "API $api is on block 10" && ((NODES_ON_10++)); \
      else \
        echo "API $api is not yet on block 10"; \
      fi; \
    done; \
  done;'
echo "All nodes have reached block 10!"

echo "Restarting alpha node"
docker start $(docker ps -a | grep alpha | awk {'print $1'}) > /dev/null

echo "Waiting for all nodes to reach block 15"
docker exec $ADMIN bash -c '\
  APIS=$(cd /shared_data/rest_apis && ls -d *); \
  NODES_ON_15=0; \
  until [ "$NODES_ON_15" -eq 3 ]; do \
    NODES_ON_15=0; \
    sleep 15; \
    for api in $APIS; do \
      BLOCK_LIST=$(sawtooth block list --url "http://$api:8008" \
        | cut -f 1 -d " "); \
      if [[ $BLOCK_LIST == *"15"* ]]; then \
        echo "API $api is on block 15" && ((NODES_ON_15++)); \
      else \
        echo "API $api is not yet on block 15"; \
      fi; \
    done; \
  done;'
echo "All nodes have reached block 15!"

echo "Done testing; shutting down all containers"
docker-compose -f ../adhoc/workload.yaml down && \
docker-compose -p alpha -f ../adhoc/node.yaml down && \
docker-compose -p beta -f ../adhoc/node.yaml down && \
docker-compose -p gamma -f ../adhoc/node.yaml down && \
docker-compose -f ../adhoc/admin.yaml down
