let Web3 = require("aion-web3");
let web3 = new Web3(new Web3.providers.HttpProvider("http://127.0.0.1:9001"));

const numberOfLatestBlocks = 128;

web3.eth.getBlockNumber().then(res => {
    const latestBlockNumber = res;
    console.log("Latest block number: " + latestBlockNumber);

    let promises = [];
    for (let i = 0; i < numberOfLatestBlocks && latestBlockNumber - i > 0; i++) {
        promises.push(
            new Promise(function(resolve, reject) {
                web3.eth.getBlock(latestBlockNumber - i).then(res => {
                    // console.log("Block: #" + res.number + ", " + res.sealType + ", difficulty = " + res.difficulty);
                    resolve([res.number, res.sealType, res.difficulty]);
                });
            })
        );
    }

    Promise.all(promises).then(function(values) {
        console.log("Proof-of-work difficulty")
        for (let i = values.length - 1; i >= 0; i--) {
            if (values[i][1] == "Pow") {
                console.log(values[i][0] + "," + values[i][2]);
            }
        }
        console.log("Proof-of-stake difficulty")
        for (let i = values.length - 1; i >= 0; i--) {
            if (values[i][1] == "Pos") {
                console.log(values[i][0] + "," + values[i][2]);
            }
        }
    });
});