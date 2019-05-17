let Web3 = require("aion-web3");
let web3 = new Web3(new Web3.providers.HttpProvider("http://127.0.0.1:9001"));

let args = process.argv.slice(2);
if (args.length != 1) {
    console.log("Usage: node total_difficulty_new.js [fork block (exclusive)]");
    return;
}

web3.eth.getBlockNumber().then(res => {
    const latestBlockNumber = res;
    console.log("Latest block number: " + latestBlockNumber);

    let start = parseInt(args[0]) + 1;
    let end = latestBlockNumber;

    let promises = [];
    for (let i = start; i <= end; i++) {
        promises.push(web3.eth.getBlock(i));
    }

    Promise.all(promises).then(function(values) {
        console.log("Total difficulty:")
        let tdw = 1;
        let tds = 1;
        for (let i = 0; i < values.length; i++) {
            let block = values[i];
            if (block.sealType == "Pow") {
                tdw += parseInt(block.difficulty);
            } else {
                tds += parseInt(block.difficulty);
            }
            console.log(block.timestamp, tdw * tds);
        }
    });
});