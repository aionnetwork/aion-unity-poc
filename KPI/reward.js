var Web3 = require('aion-web3');
const math = require("mathjs");
var args = process.argv.slice(2);

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

let count = {}
let totalPos = 0
let totalPow = 0

/*
    node rewards.js [start block number] [end block number]
*/

node_1.eth.getBlockNumber().then((num) => {
    console.log("current block number: " + num)
    if (args[1] > num) {
        console.log("range larger than current block number")
        return
    }
    getBlock(args[0], args[1])
})

function getBlock(start, end) {
    var promises = []
    for (var i = start; i <= end; i++) {
        promises.push(node_1.eth.getBlock(i).then( (block) => {
            if (block.sealType == 'Pos') {
                totalPos++
            } else if (block.sealType == 'Pow') {
                totalPow++
            }
            if (count[block.miner] == null) {
                count[block.miner] = 1
            } else {
                count[block.miner] += 1
            }
        }))
    }

    Promise.all(promises).then(res => { 
        let total = totalPos + totalPow
        console.log('total pos:' + totalPos + " --- " + totalPos/total*100 + '%')
        console.log('total pow:' + totalPow + " --- " + totalPow/total*100 + '%')
        for (var key in count) {
            console.log(key + ", " + count[key] + ", " + getPercentage(count[key], total));
        }
    })
}

function getPercentage(n, total) {
    return n/total*100 + '%'
}