pragma solidity ^0.4.15;

contract StakingRegistry {
    
    address private owner;
    
    uint private baseTarget;
    bytes32 private seed;
    
    mapping(address => bool) public stakers;
    mapping(address => uint) private votes;
    mapping(address => mapping (address => uint)) private stakes;
    
    event VoteChange(address indexed stakingAddress, uint totalVote);
    
    function StakingRegistry() {
        owner = msg.sender;
        baseTarget = 1;
    }
    
    function register(address stakingAddress) {
        require(msg.sender == stakingAddress);
        stakers[stakingAddress] = true;
    }
    
    function vote(address stakingAddress) public payable {
        require(stakers[stakingAddress]);
        votes[stakingAddress] += msg.value;
        stakes[msg.sender][stakingAddress] += msg.value;
        VoteChange(stakingAddress, votes[stakingAddress]);
    }
    
    function unvote(address stakingAddress, uint amount) public {
        uint currAmount = stakes[msg.sender][stakingAddress];
        uint currVote = votes[stakingAddress];
        if (amount < currAmount) {
            currAmount -= amount;
            currVote -= amount;
            votes[stakingAddress] = currVote;
            stakes[msg.sender][stakingAddress] = currAmount;
            msg.sender.transfer(amount);
        } else {
            votes[stakingAddress] = currVote - currAmount;
            stakes[msg.sender][stakingAddress] = 0;
            msg.sender.transfer(currAmount);
        }
        VoteChange(stakingAddress, votes[stakingAddress]);
    }
    
    function getVote(address stakingAddress) returns (uint) {
        return votes[stakingAddress];
    }
    
    function getBaseTarget() public returns (uint) {
        return baseTarget;
    }
    
    function setBaseTarget(uint newTarget) {
        baseTarget = newTarget;
    }
    
    function getSeed() public returns (bytes32) {
        return seed;
    }
    
    function setSeed(bytes32 newSeed) public {
        seed = newSeed;
    }
}