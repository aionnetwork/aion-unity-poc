let Web3 = require('aion-web3');

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
getTotalDiffFork(nodes[0], nodes[2]);

function getTotalDiffFork(node1, node2) {
    node1.eth.getBlockNumber().then(number1 => {
        node2.eth.getBlockNumber().then(number2 => {

            const latestBlockNumber = Math.min(number1, number2);
            console.log("Latest block number: " + latestBlockNumber);

            let start = args[0] ? parseInt(args[0]) : latestBlockNumber - numberOfLatestBlocks + 1;
            let end = args[1] ? parseInt(args[1]) : latestBlockNumber;
            start = Math.max(start, 1);
            end = Math.min(end, latestBlockNumber);
            console.log("Fetching data from block #" + start + " to #" + end);

            let promises1 = [];
            let promises2 = [];
            for (let i = start; i <= end; i++) {
                promises1.push(node1.eth.getBlock(i));
                promises2.push(node2.eth.getBlock(i));
            }

            Promise.all(promises1).then(blocks1 => {
                Promise.all(promises2).then(blocks2 => {

                    let fork = -1;
                    for (let i = 0; i < Math.min(blocks1.length, blocks2.length); i++) {
                        if (blocks1[i].hash != blocks2[i].hash) {
                            fork = i - 1;
                            break;
                        }
                    }
                    if (fork != -1) {
                        console.log("Last common block: " + blocks1[fork].number);
                    } else {
                        console.log("There is no fork");
                        return;
                    }

                    console.log("Total difficulty (chain 1):")
                    let tdw = 1;
                    let tds = 1;
                    for (let i = fork + 1; i < blocks1.length; i++) {
                        if (blocks1[i].sealType == "Pow") {
                            tdw += parseInt(blocks1[i].difficulty);
                        } else {
                            tds += parseInt(blocks1[i].difficulty);
                        }
                        console.log(blocks1[i].timestamp, tdw * tds, blocks1[i].sealType);
                    }

                    console.log("Total difficulty (chain 2):")
                    tdw = 1;
                    tds = 1;
                    for (let i = fork + 1; i < blocks2.length; i++) {
                        if (blocks2[i].sealType == "Pow") {
                            tdw += parseInt(blocks2[i].difficulty);
                        } else {
                            tds += parseInt(blocks2[i].difficulty);
                        }
                        console.log(blocks2[i].timestamp, tdw * tds, blocks2[i].sealType);
                    }
                });
            });
        });
    });
}