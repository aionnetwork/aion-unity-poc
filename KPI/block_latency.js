let Web3 = require('aion-web3');

let urls = [];

// localhost
// urls.push("http://127.0.0.1:8545");

// 4 node cluster
urls.push("http://127.0.0.1:9001");
urls.push("http://127.0.0.1:9002");
urls.push("http://127.0.0.1:9003");
urls.push("http://127.0.0.1:9004");

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
getBlockImportLatency(nodes)

// Get block import latency per node
function getBlockImportLatency(nodes) {
    nodes[0].eth.getBlockNumber().then(res => {
        const latestBlockNumber = res;
        console.log("Latest block number: " + latestBlockNumber);

        let start = args[0] ? parseInt(args[0]) : latestBlockNumber - numberOfLatestBlocks + 1;
        let end = args[1] ? parseInt(args[1]) : latestBlockNumber;
        start = Math.max(start, 1);
        end = Math.min(end, latestBlockNumber);
        console.log("Fetching data from block #" + start + " to #" + end);

        // var nodeLatency = {}
        var networkLatency = -1;
        var networkLatencyBlockCount = 0;
        for (let i = start; i <= end; i++) {
            var promises = []
            for (node of nodes) {
                promises.push(node.eth.getBlock(i)) // get all blocks except the genesis
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
                if (count > 1) {
                    averageImportLatency = (totalImportLatency / (count - 1)).toFixed()
                    networkLatency = (networkLatency * networkLatencyBlockCount + averageImportLatency) / (networkLatencyBlockCount + 1)
                    networkLatencyBlockCount++
                }
                console.log("block " + block.number + " average import latency: " + averageImportLatency + " ms")
            })
        }
    })
}