# Network Partition Simulation Using Blockade

## Requirements

- docker (>= 1.4.0 due to docker-py)
- iproute2 tools (ip and tc specifically)

## Install the blockade

```
pip3 install blockade
```

## Create a docker image

```
(cd .. && cargo build && cp target/debug/aion blockade/)
sudo docker build -t unity:latest .
```

## Launch the network

```
blockade up
```