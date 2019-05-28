let Web3 = require('aion-web3');
let math = require('mathjs');

let urls = [];

// localhost
// urls.push("http://127.0.0.1:8545");

// 4 node cluster
urls.push("http://127.0.0.1:9001");
urls.push("http://127.0.0.1:9001");
urls.push("http://127.0.0.1:9001");
urls.push("http://127.0.0.1:9001");

// 16 node cluster
// for (let i = 1; i <= 16; i++) {
//     urls.push("http://10.0.4.47:" + (19000 + i));
// }

let nodes = [];
urls.forEach(u => {
    nodes.push(new Web3(new Web3.providers.HttpProvider(u)));
});

let args = process.argv.slice(2);
const numberOfLatestBlocks = 100;

//=========================
// MAIN HERE
//=========================
getBlockTime(nodes[0])

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