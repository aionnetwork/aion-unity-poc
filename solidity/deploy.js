var Web3 = require('aion-web3');
const fs = require('fs');


web3 = new Web3(new Web3.providers.HttpProvider('http://127.0.0.1:8545'));
personal = web3.eth.personal;

// Read the solidity program from file
let stakingSol = fs.readFileSync("staking_registry.sol", "utf8");
web3.eth.compileSolidity(stakingSol).then((res) => depoly(res)); 

function depoly(compiled) {
    console.log(compiled)


    web3.eth.personal.unlockAccount('0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C', 'password', 500).then(
        (res) => callMethod(res, compiled)
    );
}

function callMethod(res, compiled) {

    // let contractInst = new web3.eth.Contract(compiled.StakingRegistry.info.abiDefinition);

    let contractInst = new web3.eth.Contract(compiled.StakingRegistry.info.abiDefinition, '0xA05c87D3cfA28a3b2aDae9E4e6e4A850A27B15A252fEcb076650ee28D726dc31', 
        {gasPrice: '10000000000', defaultAccount: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C'});


    // contractInst.methods.register('0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C').estimateGas(
    //     {from: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C'}
    // ).then((gasAmount) => send(contractInst, gasAmount));

    contractInst.methods.vote('0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C').estimateGas(
        {from: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C'}
    ).then((gasAmount) => send(contractInst, gasAmount*2));

}

function send(contractInst, gasAmount) {
    console.log("gas amount " + gasAmount);
    // contractInst.methods.register('0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C').send(
    //     {from: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C', gas: gasAmount}
    // ).then(console.log).catch(console.log)

    contractInst.methods.vote('0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C').send(
        {from: '0xa00a2D0D10ce8a2EA47A76fBb935405df2a12b0e2BC932F188F84b5f16da9C2C', gas: gasAmount, value: 50}
    ).then(console.log).catch(console.log)
}

