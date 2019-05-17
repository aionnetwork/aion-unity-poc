let Web3 = require("aion-web3");
let web3 = new Web3(new Web3.providers.HttpProvider("http://127.0.0.1:9001"));

const numberOfLatestBlocks = 1000;

web3.eth.getBlockNumber().then(res => {
    const latestBlockNumber = res;
    var blockNumber = 0;
    if (latestBlockNumber > numberOfLatestBlocks) {
    	blockNumber = latestBlockNumber - numberOfLatestBlocks
    }
    // eth_getBlockTransactionCount is modified temporarily to return orphaned block count
    web3.eth.getBlockTransactionCount(blockNumber).then(res => {
    	console.log("block height: " + latestBlockNumber)
    	console.log(res + " orphaned blocks found in the latest " + (latestBlockNumber - blockNumber) + " blocks")
    	console.log("orphaned block rate: " + res / (latestBlockNumber - blockNumber))
    })
});