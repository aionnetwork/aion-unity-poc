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

let stat = {}

let args = process.argv.slice(2);
const numberOfLatestBlocks = 100;

getRewardInfoFrom(node_1)
getRewardInfoFrom(node_2)
// getRewardInfoFrom(node_3)
// getRewardInfoFrom(node_4)

function url(node) {
    return "[" + node.currentProvider.host + "]";
}

function getRewardInfoFrom(node) {
    stat[url(node)] = {count:{}, totalPos:0, totalPow:0}
    node.eth.getBlockNumber().then(res => {
        const latestBlockNumber = res;
        console.log(url(node) + "Latest block number: " + latestBlockNumber);

        let start = args[0] ? parseInt(args[0]) : latestBlockNumber - numberOfLatestBlocks + 1;
        let end = args[1] ? parseInt(args[1]) : latestBlockNumber;
        start = Math.max(start, 1);
        end = Math.min(end, latestBlockNumber);
        console.log(url(node) + "Fetching data from block #" + start + " to #" + end);

        getBlock(node, start, end)
    })
}

function getBlock(node, start, end) {
    var promises = []
    for (var i = start; i <= end; i++) {
        promises.push(node.eth.getBlock(i).then( (block) => {
            if (!stat[url(node)].count[block.miner]) {
                stat[url(node)].count[block.miner] = {num: 0, type: block.sealType}
            }
            stat[url(node)].count[block.miner].num += 1
            
            if (block.sealType == 'Pos') {
                stat[url(node)].totalPos++
            } else if (block.sealType == 'Pow') {
                stat[url(node)].totalPow++
            }

        }))
    }

    Promise.all(promises).then(res => { 
        let total = stat[url(node)].totalPos + stat[url(node)].totalPow
        console.log(url(node) +'total pos:' + stat[url(node)].totalPos + " --- " + stat[url(node)].totalPos/total*100 + '%')
        console.log(url(node) +'total pow:' + stat[url(node)].totalPow + " --- " + stat[url(node)].totalPow/total*100 + '%')
        console.log("                                  Key                           \t Type \t Total Block \t %overall")
        for (var key in stat[url(node)].count) {
            console.log(url(node) + key + "\t" + stat[url(node)].count[key].type + "\t" + stat[url(node)].count[key].num + "\t" +  getPercentage(stat[url(node)].count[key].num, total));
        }
    })
}

function getPercentage(n, total) {
    return n/total*100 + '%'
}