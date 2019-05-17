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

let count = {}
let totalPos = 0
let totalPow = 0

let args = process.argv.slice(2);
const numberOfLatestBlocks = 100;

getRewardInfoFrom(node_1)

function getRewardInfoFrom(node) {
    node.eth.getBlockNumber().then(res => {
        const latestBlockNumber = res;
        console.log("Latest block number: " + latestBlockNumber);

        let start = args[0] ? parseInt(args[0]) : latestBlockNumber - numberOfLatestBlocks + 1;
        let end = args[1] ? parseInt(args[1]) : latestBlockNumber;
        start = Math.max(start, 1);
        end = Math.min(end, latestBlockNumber);
        console.log("Fetching data from block #" + start + " to #" + end);

        getBlock(node, start, end)
    })
}

function getBlock(node, start, end) {
    var promises = []
    for (var i = start; i <= end; i++) {
        promises.push(node.eth.getBlock(i).then( (block) => {
            if (count[block.miner] == null) {
                count[block.miner] = {num: 0, type: block.sealType}
            }
            count[block.miner].num += 1
            
            if (block.sealType == 'Pos') {
                totalPos++
            } else if (block.sealType == 'Pow') {
                totalPow++
            }

        }))
    }

    Promise.all(promises).then(res => { 
        let total = totalPos + totalPow
        console.log('total pos:' + totalPos + " --- " + totalPos/total*100 + '%')
        console.log('total pow:' + totalPow + " --- " + totalPow/total*100 + '%')
        console.log("                                  Key                           \t Type \t Total Block \t %overall")
        for (var key in count) {
            console.log(key + "\t" + count[key].type + "\t" + count[key].num + "\t" +  getPercentage(count[key].num, total));
        }
    })
}

function getPercentage(n, total) {
    return n/total*100 + '%'
}