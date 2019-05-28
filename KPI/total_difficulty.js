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
getTotalDiff(nodes[0]);

function getTotalDiff(node) {
    node.eth.getBlockNumber().then(res => {
        const latestBlockNumber = res;
        console.log("Latest block number: " + latestBlockNumber);

        let start = args[0] ? parseInt(args[0]) : latestBlockNumber - numberOfLatestBlocks + 1;
        let end = args[1] ? parseInt(args[1]) : latestBlockNumber;
        start = Math.max(start, 1);
        end = Math.min(end, latestBlockNumber);
        console.log("Fetching data from block #" + start + " to #" + end);

        let promises = [];
        for (let i = start; i <= end; i++) {
            promises.push(
                new Promise(function (resolve, reject) {
                    node.eth.getBlock(i).then(res => {
                        // console.log("Block: #" + res.number + ", " + res.sealType + ", " + res.timestamp + ", td = " + res.totalDifficulty);
                        resolve([res.number, res.sealType, res.timestamp, res.totalDifficulty]);
                    });
                })
            );
        }

        Promise.all(promises).then(function (values) {
            console.log("Total difficulty:")
            for (let i = 0; i < values.length; i++) {
                console.log(values[i][0] + "," + values[i][2] + "," + values[i][3]);
            }
        });
    });
}