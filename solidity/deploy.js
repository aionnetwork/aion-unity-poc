var Web3 = require('aion-web3');
const fs = require('fs');
var args = process.argv.slice(2);

/*
node deploy
node call register [address]
node call vote [address] [value]
*/

console.log("args:" + args);
let nodeUrl = 'http://127.0.0.1:9001';
let accountAddress = '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C';
//let contractAddress = '0xA05c87D3cfA28a3b2aDae9E4e6e4A850A27B15A252fEcb076650ee28D726dc31'
let contractAddress = '0xA00876bE75B664DE079B58E7acBf70CE315Ba4aAa487F7DdF2Abd5e0e1A8dFf4'

let defaultPassword = 'password';
web3 = new Web3(new Web3.providers.HttpProvider(nodeUrl));
personal = web3.eth.personal;

// Read the solidity program from file

let stakingSol = fs.readFileSync("staking_registry.sol", "utf8");
web3.eth.compileSolidity(stakingSol).then((res) => unlockAccount(res));

function unlockAccount(compiled) {
    web3.eth.personal.unlockAccount( accountAddress, defaultPassword, 5000)
    .then(processCommand(compiled)).catch(console.log)
}

function processCommand(compiled) {
    if (args[0] == 'deploy') {
        depoly(compiled);
    } else if (args[0] == 'call') {
        let contractInst = new web3.eth.Contract(compiled.StakingRegistry.info.abiDefinition, contractAddress, 
        {gasPrice: '10000000000', defaultAccount: accountAddress});
        callMethod(contractInst);
    }
}

function depoly(compiled) {
    console.log(compiled)
    console.log('deploying contract...');
    web3.eth.sendTransaction({from: accountAddress, 
    data: compiled.StakingRegistry.code})
    .then( res => console.log('deployed to address:'+ res.contractAddress))
    .catch(console.log)
}

function callMethod(contractInst) {

    if (args[1] == 'register') {
        let registerAccount = args[2];
        if (registerAccount === undefined) {
            registerAccount = accountAddress;
        }
        console.log("register account:" +  registerAccount);
        let method = contractInst.methods.register(registerAccount);
        method.estimateGas(
            {from: accountAddress}
            ).then((gasAmount) => send(method, gasAmount*2, null));
    } else if (args[1] == 'vote') {
        console.log("voting for: " + args[2] + " value: " + args[3] );
        let method = contractInst.methods.vote(args[2]);
        method.estimateGas({from: accountAddress}
        ).then((gasAmount) => send(method, gasAmount*2, args[3])).catch(console.log);
    } else if (args[1] == 'getvote') {
        console.log("get vote for: " + args[2]);
        let method = contractInst.methods.getVote(args[2]);
        method.estimateGas(
            {from: accountAddress}
        ).then((gasAmount) => call(method, gasAmount*2));
    } else {
        web3.eth.getBlockNumber().then((num) => {
            console.log("current block number: " + num)
            getBlock(accountAddress, num-30, num-20)
        })
    }
}

function call(method, gasAmount) {
    console.log("gas amount " + gasAmount);
    method.call(
        {from: accountAddress, gas: gasAmount}
    ).then(console.log).catch(console.log)
}

function send(method, gasAmount, value) {
    console.log("gas amount " + gasAmount + " value: " + value);
    method.send(
        {from: accountAddress, gas: gasAmount, value: value}
    ).then(console.log).catch(console.log)
}

function getBlock(address, start, end) {
    for (var i = start; i <= end; i++) {
        web3.eth.getBlock(i).then( (block) => {
            console.log(block);
        });
    }
}

