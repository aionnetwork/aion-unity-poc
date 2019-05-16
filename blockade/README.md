# Network Partition Simulation Using Blockade

## Requirements

- docker (>= `1.4.0` due to docker-py)
- iproute2 tools (`ip` and `tc` specifically)

## Install the blockade

```
pip install blockade
```

## Create a docker image

You will need to rebuild the docker image when the kernel is updated.

```
(cd .. && cargo build && cp target/debug/aion blockade/)
sudo docker build -t unity:latest .
```

## Launch the network

Before you launch the network, make sure the docker service is running 
and your user has access to the Docker API. 
Check [here](https://stackoverflow.com/questions/21871479/docker-cant-connect-to-docker-daemon)
for instruction. An easy command to test is
```
docker run unity
```

Also, you need to modify the paths in `blockade.yml`. This is required because
the docker mount paths must be absolute.

Finally, you can start the network by executing

```
blockade up
```

## Miner and staker addresses

Miners:
```
node1: 0x1111111111111111111111111111111111111111111111111111111111111111
node2: 0x2222222222222222222222222222222222222222222222222222222222222222
node3: 0x3333333333333333333333333333333333333333333333333333333333333333
node4: 0x4444444444444444444444444444444444444444444444444444444444444444
```

Stakers:
```
node1: 0xa0bd75fcd7676504671ee75f95e1ed7ada6d168d1b852956568e1a32ce6a7886
node2: 0xa08c895fc144884e989a32a7cbfbf47346ad3926f57635a10f10c24b09135ca3
node3: 0xa06586f27e6c4e218183cde720931b35056d3857b52b8aa28afbf0db110cac03
node4: 0xa0d9342bc958587c8f14781eb6b124f68336d3921732a111343f11df0e3f13fb
```

## Set up the PoW mining

Use the CPU aion miner (change the port number to connect to a different node):
```
./aionminer -l 127.0.0.1:8001 -u 0xa0a95e710948d17a33c9bab892b33f7b25da064d3109f12ac89f249291b5dcd9 -d 0 -t 1
```

## Set up the PoS mining

1. Deploy the staking registry contract, using script at `../solidity/` (Note:
you may need to change the port number):

    ```
    node deploy.js deploy
    ```

2. Register the staker (addresses are listed above):

    ```
    node deploy.js call register [address]
    ```

3. Vote for the staker

    ```
    node deploy.js call vote [address] [value]
    ```

## Check the network

- To see the standard output of a node, attack to the docker, e.g.,

    ```
    docker logs blockade_n1
    ```

- To attach a web3 console to a node, run:

    ```
    node console.js 127.0.0.1:9001
    ```
    
- To delete the databases

    ```
    sudo rm -fr node*/databases
    ```

## Partition the network

Your can partition the network into two groups by

```
blockade partition n1,n2
```

Or, apply a random partition:
```
blockade random-partition
```