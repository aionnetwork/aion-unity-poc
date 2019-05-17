let Web3 = require("aion-web3");
let web3 = new Web3(new Web3.providers.HttpProvider("http://127.0.0.1:9001"));

let args = process.argv.slice(2);
const numberOfLatestBlocks = 100;

web3.eth.getBlockNumber().then(res => {
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
            new Promise(function(resolve, reject) {
                web3.eth.getBlock(i).then(res => {
                    // console.log("Block: #" + res.number + ", " + res.sealType + ", difficulty = " + res.difficulty);
                    resolve([res.number, res.sealType, res.difficulty]);
                });
            })
        );
    }

    Promise.all(promises).then(function(values) {
        console.log("Proof-of-work difficulty:")
        for (let i = values.length - 1; i >= 0; i--) {
            if (values[i][1] == "Pow") {
                console.log(values[i][0] + "," + values[i][2]);
            }
        }
        console.log("Proof-of-stake difficulty:")
        for (let i = values.length - 1; i >= 0; i--) {
            if (values[i][1] == "Pos") {
                console.log(values[i][0] + "," + values[i][2]);
            }
        }
    });
});