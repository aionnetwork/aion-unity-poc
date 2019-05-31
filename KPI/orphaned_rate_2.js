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
getOrphanedBlockrate(nodes[0]);

function getOrphanedBlockrate(node) {
    node.eth.getBlockNumber().then(res => {
        const latestBlockNumber = res;
        console.log("Latest block number: " + latestBlockNumber);

        let start = args[0] ? parseInt(args[0]) : latestBlockNumber - numberOfLatestBlocks + 1;
        let end = args[1] ? parseInt(args[1]) : latestBlockNumber;
        start = Math.max(start, 1);
        end = Math.min(end, latestBlockNumber);
        console.log("Fetching data from block #" + start + " to #" + end);

        // eth_getBlockTransactionCount is modified temporarily to return orphaned block count
        let promises1 = [];
        let promises2 = [];
        let promises3 = [];
        for (let i = start; i <= end; i++) {
            promises1.push(node.eth.getBlock(i));
        }
        promises2.push(node.eth.getAccounts());
        promises3.push(node.eth.personal.getAccounts());

        Promise.all(promises1).then(function(values1) {
            Promise.all(promises2).then(function(values2) {
                Promise.all(promises3).then(function(values3) {
                    var hashesPOW = values2[0];
                    var hashesPOS = values3[0];
                    var mainHashesInRange = [];
                    for (value of values1) {
                        mainHashesInRange.push(value.hash)
                    }
                    
                    // To lower case
                    for (i in hashesPOW) {
                        hashesPOW[i] = hashesPOW[i].toLowerCase();
                    }
                    for (i in hashesPOS) {
                        hashesPOS[i] = hashesPOS[i].toLowerCase();
                    }

                    // Discard pow & pos hashes not in range
                    var indexStartPoW = hashesPOW.length;
                    var indexEndPoW = 0;
                    var indexStartPoS = hashesPOS.length;
                    var indexEndPoS = 0;
                    for (hash of mainHashesInRange) {
                        var index = hashesPOW.indexOf(hash);
                        if (index != -1) {
                            indexStartPoW = index;
                            break;
                        }
                    }
                    for (hash of mainHashesInRange.reverse()) {
                        var index = hashesPOW.indexOf(hash);
                        if (index != -1) {
                            indexEndPoW = index + 1;
                            break;
                        }
                    }
                    mainHashesInRange.reverse();
                    for (hash of mainHashesInRange) {
                        var index = hashesPOS.indexOf(hash);
                        if (index != -1) {
                            indexStartPoS = index;
                            break;
                        }
                    }
                    for (hash of mainHashesInRange.reverse()) {
                        var index = hashesPOS.indexOf(hash);
                        if (index != -1) {
                            indexEndPoS = index + 1;
                            break;
                        }
                    }
                    mainHashesInRange.reverse();
                    hashesPOW = hashesPOW.slice(indexStartPoW, indexEndPoW);
                    hashesPOS = hashesPOS.slice(indexStartPoS, indexEndPoS);

                    // Remove main chain hashes
                    hashesPOW = hashesPOW.filter(e => !mainHashesInRange.includes(e));
                    hashesPOS = hashesPOS.filter(e => !mainHashesInRange.includes(e));

                    let mainBlockCount = mainHashesInRange.length;
                    let powOrphanCount = hashesPOW.length;
                    let posOrphanCount = hashesPOS.length;
                    let orphanCount = powOrphanCount + posOrphanCount;
                    console.log("Main chain blocks count: " + mainBlockCount);
                    console.log("Orpahned blocks count: " + orphanCount + " POW: " + powOrphanCount + " POS: " + posOrphanCount);
                    console.log("Orpahned blocks rate: " + (orphanCount / mainBlockCount).toFixed(3)  + " POW: " + (powOrphanCount / orphanCount).toFixed(2) + " POS: " + (posOrphanCount / orphanCount).toFixed(2));
                })
            })
        })
    });
}