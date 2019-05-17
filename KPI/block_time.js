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

const nodes = [node_1, node_2, node_3, node_4]

let args = process.argv.slice(2);
const numberOfLatestBlocks = 100;

getBlockTime(node_1)

// Get block time
function getBlockTime(node) {
    node.eth.getBlockNumber().then(res => {
        const latestBlockNumber = res;
        console.log("Latest block number: " + latestBlockNumber);

        let start = args[0] ? parseInt(args[0]) : latestBlockNumber - numberOfLatestBlocks + 1;
        let end = args[1] ? parseInt(args[1]) : latestBlockNumber;
        start = Math.max(start, 1);
        end = Math.min(end, latestBlockNumber);
        console.log("Fetching data from block #" + start + " to #" + end);

        var promises = []
        for (let i = start; i <= end; i++) {
            promises.push(node.eth.getBlock(i));
        }

        Promise.all(promises).then(res => {
            blocks = res;
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
    var prevBlockTimestamp = -1
    var blockTimeMean = -1
    var blockTimeStd = -1
    for (i = 0; i < blocks.length; i++) {
        if (sealType && blocks[i].sealType != sealType) continue
        const timestamp = blocks[i].timestamp
        if (prevBlockTimestamp != -1) {
            var blockTime =  timestamp - prevBlockTimestamp;
            blockTimes.push(blockTime);
        }
        prevBlockTimestamp = timestamp;
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