FROM ubuntu:18.04

# Update the sources
RUN apt-get update

# Install common utility tools
RUN apt-get install -y wget curl net-tools iputils-ping

# Install dependencies
RUN apt-get install -y libjsoncpp-dev libudev-dev llvm-4.0-dev libboost-all-dev libzmq3-dev

# Expose web3 and stratum ports
EXPOSE 30303
EXPOSE 8545
EXPOSE 8008

# Copy the binary
COPY ./aion .

# Run the kernel
#CMD ["./aion"]
