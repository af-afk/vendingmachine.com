// SPDX-License-Identifier: MIT
pragma solidity 0.8.20;

/* ~~~~~~ EVENTS ~~~~~~ */

interface IEvents {
    event LockedUpTokens(
        address indexed recipient,
        uint256 indexed amount
    );

    event RandomnessRequested(uint256 indexed ticketNo);

    event RandomnessResolved(uint256 indexed ticketNo);

    event UserReceivedNFT(
        address indexed nft,
        uint256 indexed id,
        uint256 indexed usdAmount,
        uint256 refunded,
        address recipient
    );

    event UserRebated(address indexed user, uint256 indexed amount);
}