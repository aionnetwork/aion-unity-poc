# Network Partition Simulation Using Blockade

## Requirements

- docker (>= 1.4.0 due to docker-py)
- iproute2 tools (ip and tc specifically)

## Install the blockade

```
pip install blockade
```

## Create a docker image

```
(cd .. && cargo build && cp target/debug/aion blockade/)
sudo docker build -t unity:latest .
```

## Launch the network

Before you launch the network, make sure the docker service is running 
and your user has access to the Docker API. 
Check [here](https://stackoverflow.com/questions/21871479/docker-cant-connect-to-docker-daemon)
for instruction. Also, you need to modify the paths in `blockade.yml`.

Then, start the network

```
blockade up
```

## Set up the POS staking


## Partition the network