var Web3 = require('aion-web3');

let nodeUrl_local = 'http://127.0.0.1:8545';
let nodeUrl_1 = 'http://127.0.0.1:9001';
let nodeUrl_2 = 'http://127.0.0.1:9002';
let nodeUrl_3 = 'http://127.0.0.1:9003';
let nodeUrl_4 = 'http://127.0.0.1:9004';
node_local = new Web3(new Web3.providers.HttpProvider(nodeUrl_local));
node_1 = new Web3(new Web3.providers.HttpProvider(nodeUrl_1));
node_2 = new Web3(new Web3.providers.HttpProvider(nodeUrl_2));
node_3 = new Web3(new Web3.providers.HttpProvider(nodeUrl_3));
node_4 = new Web3(new Web3.providers.HttpProvider(nodeUrl_4));

const numberOfLatestBlocks = 32;
const nodes = [node_1, node_2, node_3, node_4]

getBlockTime(node_1)
getBlockImportLatency(nodes)

// Get block import latency per node
function getBlockImportLatency(nodes) {
    nodes[0].eth.getBlockNumber().then(res => {
        const blockHeight = res
        var index = 1
        // var nodeLatency = {}
        var networkLatency = -1;
        var networkLatencyBlockCount = 0;
        while (blockHeight >= index) {
            var promises = []
            for (node of nodes) {
                promises.push(node.eth.getBlock(index)) // get all blocks except the genesis
            }
            Promise.all(promises).then(res => {
                var totalImportLatency = 0;
                var averageImportLatency = -1;
                var count = 0;
                var earliestTimestamp = Number.MAX_SAFE_INTEGER;
                for (block of res) {
                    if (block.importTimestamp < earliestTimestamp) {
                        earliestTimestamp = block.importTimestamp
                    }
                    count++
                }
                for (block of res) {
                    var importLatency = block.importTimestamp - earliestTimestamp
                    totalImportLatency += importLatency
                }
                averageImportLatency = totalImportLatency / count
                networkLatency = (networkLatency * networkLatencyBlockCount + averageImportLatency) / (networkLatencyBlockCount + 1)
                networkLatencyBlockCount++
                console.log("block " + block.number + " average import latency: " + averageImportLatency)
            })
            index++
        }
    })
}

// Get block time
function getBlockTime(node) {
    node.eth.getBlockNumber().then(res => {
        console.log("current block height: " + res)
        const blockHeight = res
        var index = 1
        var promises = []
        while (blockHeight >= index) {
            promises.push(node.eth.getBlock(index)) // get all blocks except the genesis
            index++
        }

        Promise.all(promises).then(res => {
            blocks = res.reverse();
            console.log("average block time: " + calculateAverageBlockTime(blocks))
            console.log("average pow block time: " + calculateAverageBlockTime(blocks, "Pow"))
            console.log("average pos block time: " + calculateAverageBlockTime(blocks, "Pos"))
        })
    })
}


function calculateAverageBlockTime(blocks, sealType) {
    var count = 0;
    var nextBlockTimestamp = -1;
    var totalBlockTime = 0;
    var averageBlockTime = -1;
    for (i = 0; i < blocks.length && count < numberOfLatestBlocks; i++) {
        if (sealType && blocks[i].sealType != sealType) continue
        const timestamp = blocks[i].timestamp
        if (nextBlockTimestamp != -1) {
            var blockTime =  nextBlockTimestamp - timestamp
            totalBlockTime += blockTime
        }
        nextBlockTimestamp = timestamp
        count++
        // console.log(blocks[i].number + ": " + timestamp)
    }
    if (count > 1) {
        averageBlockTime = totalBlockTime / (count - 1)
    }
    return averageBlockTime
}