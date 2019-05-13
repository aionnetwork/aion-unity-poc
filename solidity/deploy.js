var Web3 = require('aion-web3');
const fs = require('fs');
var args = process.argv.slice(2);

console.log("args:" + args);


web3 = new Web3(new Web3.providers.HttpProvider('http://127.0.0.1:8545'));
personal = web3.eth.personal;

// Read the solidity program from file
let stakingSol = fs.readFileSync("staking_registry.sol", "utf8");
web3.eth.compileSolidity(stakingSol).then((res) => unlockAccount(res));

function unlockAccount(compiled) {
    web3.eth.personal.unlockAccount('0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C', 'password', 500)
    .then(processCommand(compiled))
}

function processCommand(compiled) {
    console.log(compiled)
    if (args[0] == 'deploy') {
        depoly(compiled);
    } else if (args[0] == 'call') {
        let contractInst = new web3.eth.Contract(compiled.StakingRegistry.info.abiDefinition, '0xA05c87D3cfA28a3b2aDae9E4e6e4A850A27B15A252fEcb076650ee28D726dc31', 
        {gasPrice: '10000000000', defaultAccount: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C'});
        callMethod(contractInst);
    }
}

function depoly(compiled) {
    web3.eth.sendTransaction({from: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C', 
    data: compiled.StakingRegistry.code}).then(console.log);
}

function callMethod(contractInst) {

    if (args[1] == 'register') {
        let method = contractInst.methods.register('0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C');
        method.estimateGas(
            {from: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C'}
            ).then((gasAmount) => send(method, gasAmount*2), null);
    } else if (args[1] == 'vote') {
        let method = contractInst.methods.vote('0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C');
        method.estimateGas(
            {from: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C'}
        ).then((gasAmount) => send(method, gasAmount*2), args[2]);
    }
}

function send(method, gasAmount, value) {
    console.log("gas amount " + gasAmount);
    method.send(
        {from: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C', gas: gasAmount, value: value}
    ).then(console.log).catch(console.log)
}
