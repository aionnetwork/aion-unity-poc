var Web3 = require('aion-web3');
const math = require("mathjs");

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

const numberOfLatestBlocks = 128;
const nodes = [node_1, node_2, node_3, node_4]

getBlockTime(node_1)
getBlockImportLatency(nodes)

// Get block import latency per node
function getBlockImportLatency(nodes) {
    nodes[0].eth.getBlockNumber().then(res => {
        const blockHeight = res
        var index = 0
        // var nodeLatency = {}
        var networkLatency = -1;
        var networkLatencyBlockCount = 0;
        while (blockHeight > index && index < numberOfLatestBlocks) {
            var promises = []
            for (node of nodes) {
                promises.push(node.eth.getBlock(blockHeight - index)) // get all blocks except the genesis
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
                // console.log("block " + block.number + " average import latency: " + averageImportLatency)
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
            var blockTimeStatics = calculateBlockTimeStatics(blocks)
            var blockTimeStaticsPow = calculateBlockTimeStatics(blocks, "Pow")
            var blockTimeStaticsPos = calculateBlockTimeStatics(blocks, "Pos")
            console.log("Block time statics ->  (Mean: " + blockTimeStatics.mean + ", Std: " + blockTimeStatics.std + ")")
            console.log("Pow Block time statics ->  (Mean: " + blockTimeStaticsPow.mean + ", Std: " + blockTimeStaticsPow.std + ")")
            console.log("Pos Block time statics ->  (Mean: " + blockTimeStaticsPos.mean + ", Std: " + blockTimeStaticsPos.std + ")")
        })
    })
}


function calculateBlockTimeStatics(blocks, sealType) {
    var blockTimes = []
    var nextBlockTimestamp = -1
    var count = 0
    var blockTimeMean = -1
    var blockTimeStd = -1
    for (i = 0; i < blocks.length && count < numberOfLatestBlocks; i++) {
        if (sealType && blocks[i].sealType != sealType) continue
        const timestamp = blocks[i].timestamp
        if (nextBlockTimestamp != -1) {
            var blockTime =  nextBlockTimestamp - timestamp
            blockTimes.push(blockTime)
        }
        nextBlockTimestamp = timestamp
        count++
        // console.log(blocks[i].number + ": " + timestamp)
    }
    if (blockTimes.length > 0) {
        blockTimeMean = math.mean(blockTimes).toFixed(2)
        blockTimeStd = math.std(blockTimes).toFixed(2)
    }
    var blockTimeStatics = {
        mean: blockTimeMean,
        std: blockTimeStd
    }
    return blockTimeStatics
}