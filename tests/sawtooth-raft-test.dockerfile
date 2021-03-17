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
# ------------------------------------------------------------------------------

FROM ubuntu:bionic

RUN apt-get update \
 && apt-get install gnupg -y

RUN echo "deb [arch=amd64] http://repo.sawtooth.me/ubuntu/nightly bionic universe" >> /etc/apt/sources.list \
 && (apt-key adv --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys 44FC67F19B2466EA \
 || apt-key adv --keyserver hkp://p80.pool.sks-keyservers.net:80 --recv-keys 44FC67F19B2466EA) \
 && apt-get update \
 && apt-get install -y -q --allow-downgrades \
    apt-transport-https \
    build-essential \
    curl \
    libssl-dev \
    gcc \
    git \
    pkg-config \
    python3 \
    python3-sawtooth-cli \
    python3-sawtooth-rest-api \
    python3-sawtooth-settings \
    python3-sawtooth-validator \
    python3-requests \
    python3-nose2 \
    sawtooth-smallbank-workload \
    sawtooth-smallbank-tp-go \
    software-properties-common \
    unzip \
 # Install docker
 && curl -fsSL https://download.docker.com/linux/$(. /etc/os-release; echo "$ID")/gpg > /tmp/dkey \
 && apt-key add /tmp/dkey \
 && add-apt-repository \
   "deb [arch=amd64] https://download.docker.com/linux/$(. /etc/os-release; echo \"$ID\") \
   $(lsb_release -cs) \
   stable" \
 && apt-get update \
 && apt-get -y install docker-ce \
 # Install docker-compose
 && curl -L "https://github.com/docker/compose/releases/download/1.22.0/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose \
 && chmod +x /usr/local/bin/docker-compose \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/*

RUN curl -sL https://repos.influxdata.com/influxdb.key | apt-key add -
RUN echo deb https://repos.influxdata.com/ubuntu bionic stable > /etc/apt/sources.list.d/influxdb.list
RUN apt-get update && apt-get install -y -q \
    telegraf \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/*
