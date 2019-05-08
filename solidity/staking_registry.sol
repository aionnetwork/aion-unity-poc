pragma solidity ^0.4.15;

contract StakingRegistry {
    
    struct VoteSummary {
        uint totalVote;
        mapping (address => uint) details;
    }
    
    address private owner;
    
    uint private baseTarget;
    bytes32 private seed;
    
    mapping(address => bool) public stakers;
    mapping(address => VoteSummary) private votes;

    event RegistrationChange(address indexed stakingAddress, bool status);
    event VoteChange(address indexed stakingAddress, uint totalVote);
    
    function StakingRegistry() {
        owner = msg.sender;
        baseTarget = 1;
    }
    
    function register(address stakingAddress) {
        require(msg.sender == stakingAddress);
        stakers[stakingAddress] = true;
        RegistrationChange(stakingAddress, true);
    }
    
    function vote(address stakingAddress) public payable {
        require(stakers[stakingAddress]);
        votes[stakingAddress].totalVote += msg.value;
        votes[stakingAddress].details[msg.sender] += msg.value;
        VoteChange(stakingAddress, votes[stakingAddress].totalVote);
    }
    
    function unvote(address stakingAddress, uint amount) public {
        uint currAmount = votes[stakingAddress].details[msg.sender];
        uint currVote = votes[stakingAddress].totalVote;
        if (amount < currAmount) {
            currAmount -= amount;
            currVote -= amount;
            votes[stakingAddress].totalVote = currVote;
            votes[stakingAddress].details[msg.sender] = currAmount;
            msg.sender.transfer(amount);
        } else {
            votes[stakingAddress].totalVote = currVote - currAmount;
            votes[stakingAddress].details[msg.sender] = 0;
            msg.sender.transfer(currAmount);
        }
        VoteChange(stakingAddress, votes[stakingAddress].totalVote);
    }
    
    function getVote(address stakingAddress) returns (uint) {
        return votes[stakingAddress].totalVote;
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
