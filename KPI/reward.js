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
let stat = {}
getRewardInfoFrom(nodes[0]);
getRewardInfoFrom(nodes[1]);

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